/*!
This reasoner relies on the crate [`mod@aries_lp`] and the following paper: [A Fast Linear-Arithmetic Solver for DPLL(T)][ref-doc].

The LP solver is used to check feasibility, as it works on a continuous relaxation
of the integer problem. In parallel, an exact integer version of the problem is maintained in `Solver`
to allow the exact validation of unsat certificates (`Solver::check_certificate()`), thereby guaranteeing soundness whenever
infeasibility is detected.


Its entry point is the function [`Lp::add_linear_leq_constraint()`], which is used to post
[`crate::lang::CoreExpr::LinearLeq`] constraints to the reasoner.
It is the responsibility of the caller to linearize other types of constraints before posting
them to the LP.

The reasoner uses an underlying LP solver, using a dual-revised simplex algorithm. In many problems,
the LP relaxation can detect early that entire subtrees are infeasible. However, solving the LP (even just for feasibility)
is typically much slower than a typical propagation budget and many terribly slow down the solver
(sometime for no benefit).
To limit the downsides (while keeping the upsides), the LP reasoner operates on computational budget,
expressed as a number of pivot operations per decision of the solver.
Each time a decision is made (as witnessed by a save_state call), the LP is given a number additional pivots allowance.
When this number is small, the LP checker might not be able to provide a conclusion on the feasibility of the LP.
When that is the case, the LP will not be updated when the solver takes a new decision and the next propagate will
keep trying to determine feasibility at the previous level. It is only when the simplex establishes feasibility of a given
decision level that the LP will proceed to the next.

The consequences are thus that the LP solver as a budget that is proportional to the number of decisions.
Hence the LP is typically lagging behind the CP solver, and may detect unsatisfiability of a subtree that the
CP solver has already spent many decisions/conflict in.
THis however generally a good trade-off because the if the subtree that could be cut is very large, the LP
will eventually get enough budget to prune it (just with some delay). On the other hand, if the small tree,
is small, the CP solver will close with limited time lost on the LP solver.


> **Important:** This reasoner is **disabled by default**.
> To enable it, set the environment variable **`ARIES_LP_ENABLE=true`** or overwrite the EnvParam [`LP_ENABLE`] using [`EnvParam::set()`].

[ref-doc]: https://doi.org/10.1007/11817963_11
*/

mod explanation_utils;
mod solver;
mod sum;

pub use sum::LpSum;

use std::collections::HashMap;

use aries_env_param::EnvParam;
#[allow(unused_imports)]
use itertools::Itertools;
use solver::{BoundJustification, Solver};

use crate::prelude::*;
use aries_lp::prelude::*;

use crate::{
    backtrack::{Backtrack, DecLvl, DecisionLevelTracker, ObsTrailCursor, Trail},
    collections::ref_store::{RefMap, RefVec},
    core::{
        IntCst, Lit, LongCst, Var, cst_int_to_long,
        literals::Watches,
        state::{Domains, DomainsSnapshot, Event, Explanation, InferenceCause},
    },
    lang::linear::LinSum,
    prelude::Conjunction,
    reasoners::{Contradiction, ReasonerId, Theory},
};

/// Contains all the options available for the Lp reasoner
///
/// It can be passed when creating the reasoner or modified through following methods:
/// [`Lp::activate`], [`Lp::deactivate`], [`Lp::deactivate_propagation`], [`Lp::activate_refined_explanation`] and [`Lp::deactivate_refined_explanation`]
#[derive(Debug, Clone, Copy)]
pub struct LpOptions {
    /// Used to activate/deactivate the propagation of the reasoner
    ///
    /// Can be controlled with the following methods: [`Lp::activate`], [`Lp::deactivate`] and [`Lp::deactivate_propagation`]
    pub propagation_active: bool,
    /// Used to enable/disable the reasoner
    ///
    /// Can be controlled trough the following environment variable: ARIES_LP_ENABLE
    /// or after the creation with [`Lp::activate`] and [`Lp::deactivate`]
    pub enable: bool,
    /// If true, refined explanation are used with minimization based on [`crate::reasoners::cp::linear`]
    ///
    ///Can be controlled with the following methods: [`Lp::activate_refined_explanation`], [`Lp::deactivate_refined_explanation`]
    pub is_explanation_refined: bool,
    /// Number of pivots allowed for each decision.
    ///
    /// THe default value is controlled by [`LP_PIVOTS_PER_DECISION`].
    /// THe best value is very problem-dependent and the default is typically very small (~0.1).
    pub pivots_per_decision: f64,
}

impl Default for LpOptions {
    fn default() -> Self {
        Self {
            propagation_active: true,
            enable: LP_ENABLE.get(),
            is_explanation_refined: true,
            pivots_per_decision: LP_PIVOTS_PER_DECISION.get(),
        }
    }
}

/// Used to enable/disable the lp reasoner
///
/// Can be set either from the env variable **`ARIES_LP_ENABLE`**
/// or within the code with: `LP_ENABLE.set(true/false)`
pub static LP_ENABLE: EnvParam<bool> = EnvParam::new("ARIES_LP_ENABLE", "false");

/// Default number of pivots per decision.
pub static LP_PIVOTS_PER_DECISION: EnvParam<f64> = EnvParam::new("ARIES_LP_PIVOTS_PER_DECISION", "0.1");

/// Number of initially allowed pivots, before any decision is taken.
pub static LP_INITIALLY_ALLOWED_PIVOTS: EnvParam<f64> = EnvParam::new("ARIES_LP_INITIALLY_ALLOWED_PIVOTS", "0");

/// An upper or lower bound constraint on a single LP variable.
#[derive(Debug, Clone, Copy)]
pub struct BoundRestriction {
    var: Variable,
    bound: Bound,
    val: LongCst,
}

impl BoundRestriction {
    /// Represents an upper bound on an LP variable.
    pub fn leq(var: Variable, ub: LongCst) -> Self {
        Self {
            var,
            bound: Bound::Upper,
            val: ub,
        }
    }
    /// Represents a lower bound on an LP variable.
    pub fn geq(var: Variable, lb: LongCst) -> Self {
        Self {
            var,
            bound: Bound::Lower,
            val: lb,
        }
    }
}

/// Identifies a bound constraint in the reasoner.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct BoundConstraintId(usize);

impl From<usize> for BoundConstraintId {
    fn from(value: usize) -> Self {
        BoundConstraintId(value)
    }
}
impl From<BoundConstraintId> for usize {
    fn from(value: BoundConstraintId) -> Self {
        value.0
    }
}

type BoundConstraintsStore = RefVec<BoundConstraintId, (Conjunction, BoundRestriction)>;

/// Store all the necessary information for backtracking after modifying a bound
///
/// The old value and justification are necessary as they will overwrite the current ones
#[derive(Clone)]
struct LpEvent {
    /// variable affected by the bound change
    var: Variable,
    /// bound modified
    bound: Bound,
    /// Old value for the bound that needs to overwrite the new one when backtracking
    old_val: LongCst,
    /// Justification associated with this bound and value
    old_justification: BoundJustification,
}

#[derive(Clone)]
struct Stats {
    /// Number of propagations where the solver returned `Feasible`
    num_feasible: usize,
    /// Number of propagations
    num_propagate: usize,
    /// Number of constraints within the LP relaxation
    num_constraints: usize,
    /// Number of variables within the LP relaxation
    num_variables: usize,
    /// Number of certificates generated
    num_certif: usize,
    /// Number of valid certificates generated
    num_val_certif: usize,
    /// Number of overflows detected during the certificate verification
    num_overflow: usize,
    /// Number of proven infeasibility
    num_contradictions: usize,
    /// Number of timeouts
    num_check_timeouts: usize,
    /// Number of times the LP reported a numerical instability error.
    num_unstable: usize,
}

impl Stats {
    fn new() -> Self {
        Self {
            num_feasible: 0,
            num_propagate: 0,

            num_constraints: 0,
            num_variables: 0,

            num_certif: 0,
            num_val_certif: 0,
            num_overflow: 0,
            num_contradictions: 0,
            num_check_timeouts: 0,
            num_unstable: 0,
        }
    }
}

/// A variable of the LP solver.
pub type LpVar = aries_lp::Variable;

/// Applies a sign to a type.
enum Signed<T> {
    /// + T
    Plus(T),
    /// - T
    Minus(T),
}

/// Struct that implements the [`Theory`] trait (reasoner).
///
/// It encapsulates all the necessary information to run the lp solver on the posted constraints.
///
/// The propagation can be dynamically activated / deactivated through the following methods: [`Lp::activate()`] and [`Lp::deactivate()`].
/// This can be useful to avoid an overhead of the lp reasoner over the others if its not relevant.
#[derive(Clone)]
pub struct Lp {
    id: ReasonerId,
    /// Encapsulates both float and integer versions of our constraints and an instance of the aries-lp solver
    solver: Solver,
    /// Associates each bound constraint identifiers with a trigger (conjunction of CP literals) and a corresponding bound restriction.
    /// the constraint is applied to the LP as soon as all the literals of its trigger are entailed.
    bound_constrs: BoundConstraintsStore,
    /// Indices in `bound_constrs` of the constraints whose cause was already entailed when
    /// they were registered, and that are thus waiting to be applied at the next propagation.
    pending_bound_constrs: Vec<BoundConstraintId>,
    /// Decision level at which the pending constraints were last applied. No watch can trigger them
    /// again, so backtracking below it makes them all pending anew.
    pending_bound_constrs_applied_at: Option<DecLvl>,
    /// Associates linear sums with its corresponding reifying variable in the aries-lp solver.
    /// The sums are simplified, i.e. their terms are sorted those sharing the same var are merged.
    ///
    /// Used to avoid duplicate variables for the same sum (or its opposite).
    reifications: HashMap<LpSum, Variable>,
    /// Maps var from aries solver with their corresponding variable in minilp (if they appear in the post constraints)
    memory_x_main: RefMap<Var, Variable>,
    model_events: ObsTrailCursor<Event>,
    /// The watcher corresponds to an index in `bound_constrs_lit_vec`
    watches: Watches<BoundConstraintId>,
    /// History of changes made to the LP with all information necessary to undo them.
    ///
    /// The trail may have less saved states as the CP solver we evolve in because the reasoner is typically lagging behind due to
    /// a bounded computational effort.
    trail: Trail<LpEvent>,
    /// Mirrors the decision level of the CP solver. Incremented/decremented each time a save/restore is invoked.
    ///
    /// It differs from the logical decision level ([`Self::lp_level`]) of this reasoner (`self.trail.current_decision_level`) because we only go to the next decision level
    /// once we have determined that the LP relaxation is feasible.
    cp_decision_level: DecisionLevelTracker,
    /// Running statistics of the solver (number of propagations, conflicts, ...)
    stats: Stats,
    /// Contains all the customizable options for the reasoner
    options: LpOptions,
    /// Total number of pivots allowed for this reasoner (since its start).
    /// This is used to bound the effort of the simplex solver.
    ///
    /// It is incremented by [`LpOptions::pivots_per_decision`] on each decision (we assume 1 decision per save_state call)
    ///
    /// Setting this `INFINITY` will remove any limit on the pivots (useful for testing).
    /// Setting it `-INFINITY` will disallow any pivot operation.
    total_allowed_pivots: f64,
}

impl Default for Lp {
    fn default() -> Self {
        Self::new(LpOptions::default())
    }
}

impl Lp {
    pub fn new(options: LpOptions) -> Self {
        Self {
            id: ReasonerId::Cp,
            solver: Solver::new(options.is_explanation_refined),

            bound_constrs: Default::default(),
            pending_bound_constrs: Vec::new(),
            pending_bound_constrs_applied_at: None,

            reifications: HashMap::new(),
            memory_x_main: RefMap::default(),

            model_events: ObsTrailCursor::new(),
            watches: Default::default(),
            trail: Default::default(),
            cp_decision_level: DecisionLevelTracker::new(),

            stats: Stats::new(),

            options,
            total_allowed_pivots: LP_INITIALLY_ALLOWED_PIVOTS.get(),
        }
    }
    /// Activate propagation and constraints registration of the LP
    ///
    /// All constraints registered before the activation of the LP will be ignored
    pub fn activate(&mut self) {
        self.options.propagation_active = true;
        self.options.enable = true;
    }

    /// Deactivate propagation and constraints registration of the LP
    pub fn deactivate(&mut self) {
        self.options.propagation_active = false;
        self.options.enable = false;
    }

    /// Deactivate propagation of the LP, new constraints and variables will still be registered and used when reactivated
    pub fn deactivate_propagation(&mut self) {
        self.options.propagation_active = false;
    }

    /// Activate refined explanation using minimization based on [`crate::reasoners::cp::linear`]
    ///
    /// Explanations will be smaller in general therefore more useful but take more time to compute
    pub fn activate_refined_explanation(&mut self) {
        self.options.is_explanation_refined = true;
        self.solver.is_explanation_refined = true;
    }

    /// Deactivate refined explanation
    pub fn deactivate_refined_explanation(&mut self) {
        self.options.is_explanation_refined = false;
        self.solver.is_explanation_refined = false;
    }

    /// Creates a new variable in the LP that directly mirrors the given CP variable.
    ///
    /// Any bound change to the CP variable will be reflected in the LP.
    /// If the CP variable is already bound to an LP variable, this one is returned (no new LP variable is created in this case.).
    pub fn bind_cp_var(&mut self, solver_var: Var, doms: &Domains) -> LpVar {
        debug_assert_eq!(
            doms.current_decision_level(),
            DecLvl::ROOT,
            "the variable bounds may have evolved since root"
        );
        if let Some(lp_var) = self.memory_x_main.get(solver_var) {
            *lp_var
        } else {
            let lp_var = self.solver.create_variable(
                cst_int_to_long(doms.lb(solver_var)),
                cst_int_to_long(doms.ub(solver_var)),
                &mut self.stats,
            );
            self.memory_x_main.insert(solver_var, lp_var);
            self.solver.map_lp_to_aries.insert(lp_var.idx(), solver_var);
            lp_var
        }
    }

    /// Creates a variable in the LP that is independent of any variable in the CP solver.
    pub fn create_auxiliary_variable(&mut self, lb: IntCst, ub: IntCst) -> LpVar {
        self.solver
            .create_variable(cst_int_to_long(lb), cst_int_to_long(ub), &mut self.stats)
    }

    /// Retrieve a single variable `s` that is constrained to always be equals to the given linear `sum`.
    ///
    /// In the general case, this means creating a new LP variable `s` and impose a constraint `sum = s` in the LP.
    ///
    /// The function returns the `{+,-}s` variable where the sign is required to avoid duplicated reification variables.
    fn reify_sum(&mut self, sum: LpSum) -> Signed<LpVar> {
        // if the sum is exactly one variable, with factor +/-1, no need for a new variable to reify.
        if let Some((var, 1)) = sum.as_single_term() {
            return Signed::Plus(var);
        }
        if let Some((var, -1)) = sum.as_single_term() {
            return Signed::Minus(var);
        }

        // if already have this sum in our cache, just reuse the reification variable.
        if let Some(reif) = self.reifications.get(&sum) {
            return Signed::Plus(*reif);
        }
        let minus_sum: LpSum = sum.iter().map(|(var, factor)| (var, -factor)).into();
        if let Some(reif) = self.reifications.get(&minus_sum) {
            return Signed::Minus(*reif);
        }

        let mut constraint = vec![];

        let mut lb: LongCst = 0;
        let mut ub: LongCst = 0;

        for (var, factor) in sum.iter() {
            let (elem_lb, elem_ub) = self.solver.get_init_bounds(factor, var);
            ub = ub.saturating_add(elem_ub);
            lb = lb.saturating_add(elem_lb);

            constraint.push((var, factor));
        }

        // Create a variable `s` that we will constraint to be equal to `sum`.
        // Its bounds corresponds to the min/max value that the sum can take.
        let s = self.solver.create_variable(lb, ub, &mut self.stats);

        // create a constraint such that `sum = s`
        let mut constraint = sum.clone();
        constraint.push(s, -1);

        // We force s to be equal to our linear sum
        self.solver.add_constraint(constraint);
        self.stats.num_constraints += 1;

        self.reifications.insert(sum, s);

        Signed::Plus(s)
    }

    /// Adds a half-reified LinearLeq constraint of the form `active => sum <= 0`, where `sum` is a linear sum
    /// on CP variables and a constant term.
    ///
    /// `active` is the activation [`Lit`], the constraint is only active when it is evaluated to `true` and present.
    ///
    /// This method is convenience wrapper around [`Self::add_linear_leq_constraint`] which operates on CP variables.
    /// It will first bind any CP variable in the constraint with [`Self::bind_cp_var`] to an LP variable (creating one if necessary).
    pub fn add_cp_linear_leq_constraint(&mut self, sum: &LinSum, active: Lit, doms: &Domains) {
        let sum_cst = sum.constant();
        let sum_terms = LpSum::new(
            sum.terms_slice()
                .iter()
                .map(|&svar| (self.bind_cp_var(svar.var, doms), svar.factor)),
        );
        self.add_linear_leq_constraint(sum_terms, sum_cst, active, doms)
    }

    /// Adds an `active => sum_terms + sum_cst <= 0` constraint, where `sum_terms` consists only of LP variables.
    ///
    /// LP variables can be introduced either with [`Self::bind_cp_var`] (direct mapping from CP variables) or with
    /// [`Self::create_auxiliary_variable`] (independent variable).
    pub fn add_linear_leq_constraint(
        &mut self,
        sum_terms: impl Into<LpSum>,
        sum_cst: IntCst,
        active: Lit,
        doms: &Domains,
    ) {
        let sum_terms = sum_terms.into();
        if !self.options.enable {
            return;
        }

        let bound_val = cst_int_to_long(-sum_cst);

        // create a bound constraint (i.e. on a single variable) that is equivalent to this constraint.
        // We dot this by reifying the sum into a single variable and determine the appropriate bound on this one.
        let bound_constr = match self.reify_sum(sum_terms) {
            Signed::Plus(reif) => {
                // sum_terms = reif
                // reif <= bound_val
                BoundRestriction::leq(reif, bound_val)
            }
            Signed::Minus(reif) => {
                // sum_terms = -reif
                // sum_terms  <= bound_val
                // -reif <= bound_val
                // reif >= -bound_val
                BoundRestriction::geq(reif, -bound_val)
            }
        };

        // this constraint should be activated when `active is entailed and present
        let trigger = Conjunction::from([active, doms.presence(active)]);
        self.add_bound_update_trigger(trigger, bound_constr, doms);
    }

    /// Adds a new bound-restriction trigger of the form `l1 & ... & ln => var {<=,>=} bound` where each `li` is a CP literal, `variable is an
    /// LP variable, and `bound` is an integer value denoting  a new upper/lower bound of `variable`.
    ///
    /// This provides an additional mechanism for synchronizing the bounds of the LP variables to the domains in the CP solver.
    pub fn add_bound_update_trigger(
        &mut self,
        trigger: Conjunction,
        bound_update: BoundRestriction,
        domains: &Domains,
    ) {
        // use domains that mirror the state at the LP level.
        let domains = DomainsSnapshot::up_to(domains, self.lp_level());

        let id = self.bound_constrs.next_key();

        // check if the trigger is already entailed and in which case, enqueue for immediate propagation.
        // This is necessary because watches would only trigger for future events.
        let entailed = trigger.iter().all(|l| domains.entails(l));
        if entailed {
            self.pending_bound_constrs.push(id);
        }
        if !entailed || self.current_decision_level() != DecLvl::ROOT {
            // unless we are at ROOT AND the trigger is entailed, we place watch to we can check the trigger if one of its literals becomes true
            //
            // Note: if this becomes performance critical, we could use a single watch, always placed on an undecided literal
            // but this would complicate the implementation and unlikely to be useful in practice as the the vast majority of case should involve only one/two
            // literals in the conjunction.
            for l in &trigger {
                self.watches.add_watch(id, l);
            }
        }
        let actual_id = self.bound_constrs.push((trigger, bound_update));
        debug_assert!(id == actual_id);
    }

    /// Applies the bound constraints whose cause was already entailed when they were registered.
    fn apply_pending_bound_constrs(&mut self, domains: &DomainsSnapshot) -> Result<(), Contradiction> {
        if !self.pending_bound_constrs.is_empty() {
            debug_assert!(self.pending_bound_constrs_applied_at.is_none());
            self.pending_bound_constrs_applied_at = Some(self.current_decision_level());
        }

        while let Some(index) = self.pending_bound_constrs.pop() {
            let (cause, bound_constr) = &self.bound_constrs[index];

            let entailed = cause.iter().all(|l| domains.entails(l));

            if !entailed {
                // No longer entailed (we backtracked since the registration).
                // The watches will trigger the constraint again if its cause becomes entailed anew.
                continue;
            }

            let res = if cause.iter().all(|l| domains.entailing_level(l) == DecLvl::ROOT) {
                // A cause entailed at the root can never be undone,
                // so the bound is set permanently instead of being trailed and lost on the first backtrack below the current level.
                self.solver.set_bound_restrict_permanent(bound_constr)
            } else {
                self.solver
                    .set_bound_restrict(bound_constr, BoundJustification::Trigger(index), &mut self.trail)
            };

            if let Err(infeasible) = res {
                self.explain_infeasibility(&infeasible, domains)?;
            }
        }
        Ok(())
    }

    /// Attempts to derive a contradiction from a LP infeasibility and returns it as an error (to facilitate the propagation of the contradiction).
    ///
    /// In theory, any LP infeasibility lead to a contradiction in the solver because the LP is a relaxation of the CP model.
    /// However, the LP' simplex is using floating point arithmetic and it might the case that it hands out an incorrect result.
    ///
    /// To isolate such errors from the solver, we validate the infeasibility justification from the LP with exact integer arithmetic.
    /// This may fail when 1) the LP's output was erroneous or 2) our conversion into integers lost some precision and invalidated the certificate.
    ///
    /// In both cases (which should exceptional), we ignore the infeasibility and report that there is nothing to see.
    fn explain_infeasibility(
        &mut self,
        cause: &InfeasibilityReason,
        domains: &DomainsSnapshot,
    ) -> Result<(), Contradiction> {
        match cause {
            InfeasibilityReason::Certified(cert) => {
                self.stats.num_certif += 1;
                // The certificate was already verified by the LP solver itself (with
                // floating point arithmetic); it is cross-checked here with exact
                // integer arithmetic over the CP model.
                match self.solver.check_certificate(
                    cert.as_multipliers(),
                    domains,
                    &mut self.stats,
                    &self.bound_constrs,
                ) {
                    Some(explanation) => {
                        self.stats.num_val_certif += 1;
                        self.stats.num_contradictions += 1;
                        Err(Contradiction::Explanation(explanation))
                    }
                    None => {
                        // We could not validate this certificate, so we ignore this LP infeasibility
                        // as we are lacking a proof that it is a real UNSAT and not an internal error due to, e.g.,
                        // floating point errors.
                        self.stats.num_unstable += 1;
                        Ok(())
                    }
                }
            }
            InfeasibilityReason::IncompatibleBounds(var) => {
                let explanation = self.solver.explain_infeasible_var(*var, &self.bound_constrs);
                self.stats.num_contradictions += 1;
                Err(Contradiction::Explanation(explanation))
            }
        }
    }

    /// Decision level that LP mirrors.
    /// Cannot be greater than the solver's level.
    fn lp_level(&self) -> DecLvl {
        self.trail.current_decision_level()
    }

    /// Decision level at which the CP solver is currently at.
    fn solver_lvl(&self) -> DecLvl {
        self.cp_decision_level.current_decision_level()
    }

    /// Determines the deadline which an LP solver call must finish.
    ///
    /// Note that the deadline is expressed in a maximum number of pivots over the entire
    /// lifetime of the reasoner.
    fn lp_dealine(&self) -> aries_lp::Deadline {
        let max_pivots = if self.total_allowed_pivots == f64::INFINITY {
            usize::MAX
        } else if self.total_allowed_pivots <= 0.0 {
            0
        } else {
            self.total_allowed_pivots.floor() as usize
        };
        aries_lp::Deadline::NumPivots(max_pivots)
    }
}

impl Theory for Lp {
    fn identity(&self) -> ReasonerId {
        self.id
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        if !self.options.propagation_active || !self.options.enable {
            return Ok(());
        }

        // the domains in the CP solver. We are not allowed to operate on them but
        // must instead operate on the domains as they were on the LP level.
        let cp_domains = domains;

        // marker for whether we bump the LP level in the propagation.
        // Initially true to kickstart at least one propagation loop
        let mut increased_level = true;

        // run a loop, that will be allowed to restart if be concluded feasibility of the
        // current LP level (which is thus bumped to the next).
        while increased_level {
            increased_level = false;
            self.stats.num_propagate += 1;

            // mirror the domains of the LP level.
            let domains = DomainsSnapshot::up_to(cp_domains, self.lp_level());

            // Constraints registered with an already entailed cause are not triggered by any watch
            self.apply_pending_bound_constrs(&domains)?;

            // We process all the newly inferred literals since last propagation and
            // up to the current decision level.
            while let Some(&event) = self
                .model_events
                .pop_up_to(self.trail.current_decision_level(), cp_domains.trail())
            {
                let lit = event.new_literal();
                debug_assert!(cp_domains.entailing_level(lit) <= self.lp_level());

                // We first set the bounds associated with the active lit triggered by the newly inferred lit
                let watchers: Vec<BoundConstraintId> = self.watches.watches_on(lit).collect(); // TODO: remove allocation
                for watcher in watchers {
                    let (cause, bound_constr) = &self.bound_constrs[watcher];

                    // the fact that we got triggered means at least one literal became true.
                    // If they are all true we can proceed to enforce the bound
                    let entailed = cause.iter().all(|l| domains.entails(l));
                    if !entailed {
                        continue;
                    }

                    if let Err(infeasible) = self.solver.set_bound_restrict(
                        bound_constr,
                        BoundJustification::Trigger(watcher),
                        &mut self.trail,
                    ) {
                        self.explain_infeasibility(&infeasible, &domains)?;
                    }
                }

                let var = event.affected_bound.variable();

                // We update the bound of the corresponding variable of the lit in the lp solver (if there is one)
                if let Some(&x_var) = self.memory_x_main.get(var) {
                    // the bound holds because this very literal was inferred in the main model
                    let justification = BoundJustification::CpModel(lit);
                    let res =
                    // if we have is_plus, the constraint is of the form x <= b therefore it's an upper bound
                    if event.affected_bound.is_plus() {
                        self.solver
                            .set_bound_restrict(&BoundRestriction::leq(x_var, cst_int_to_long(event.new_upper_bound)), justification, &mut self.trail)
                    } else {
                        self.solver
                            .set_bound_restrict(&BoundRestriction::geq(x_var, cst_int_to_long(-event.new_upper_bound)), justification, &mut self.trail)
                    };

                    if let Err(infeasible) = res {
                        self.explain_infeasibility(&infeasible, &domains)?;
                    }
                }
            }

            // check feasibility with a maximum number of pivot operation.
            let feas_result = self.solver.check_feasibility(self.lp_dealine());

            match feas_result {
                Ok(FeasibilityStatus::Feasible) => {
                    // we proved feasibility \o/
                    // we are allowed to go to next level (if there is already one)
                    if self.solver_lvl() > self.lp_level() {
                        self.trail.save_state(); // bump LP level
                        increased_level = true // enable a new round of propagation
                    }
                    debug_assert!(self.lp_level() <= self.solver_lvl());
                    self.stats.num_feasible += 1;
                }
                Ok(FeasibilityStatus::Infeasible(cause)) => {
                    // infeasibility in the LP, compute a contraction and short-circuit
                    self.explain_infeasibility(&cause, &domains)?;
                    // if we are here, it means we could not validate the infeasibility T_T
                    // TODO: right now we do nothing but we may want to take some measure
                    // to improve numerical stability or decrease the computational effort
                }
                Err(SolverFailure::Timeout) => {
                    // not enough time allocated to the LP, we will continue later
                    self.stats.num_check_timeouts += 1;
                }
                Err(SolverFailure::Unstable) => {
                    // solver had numerical problems and could not reach a conclusion
                    // TODO: see above, we may want to take measures to improve results or reduce cost
                    self.stats.num_unstable += 1;
                }
            }
        }

        Ok(())
    }

    // Should not be called as this reasoner never infers new lit, it only gives contradictions
    fn explain(
        &mut self,
        _literal: Lit,
        _context: InferenceCause,
        _state: &DomainsSnapshot,
        _out_explanation: &mut Explanation,
    ) {
        unreachable!()
    }

    fn print_stats(&self) {
        if self.options.enable {
            println!("# propagations: {}", self.stats.num_propagate);
            println!("# contradictions: {}", self.stats.num_contradictions);
            println!("# constraints: {}", self.stats.num_constraints);
            println!("# variables: {}", self.stats.num_variables);
            println!(
                "# certificates: {}, valid: {}, overflow: {}",
                self.stats.num_certif, self.stats.num_val_certif, self.stats.num_overflow
            );
            println!("# pivots: {}", self.solver.lp.solver_stats().num_pivots);
            println!("# timeouts: {}", self.stats.num_check_timeouts);
            println!("# unstable: {}", self.stats.num_unstable);
        } else {
            println!("DISABLED");
        }
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

impl Backtrack for Lp {
    fn save_state(&mut self) -> DecLvl {
        // bump number of allowed pivots
        self.total_allowed_pivots += self.options.pivots_per_decision;

        // track solver level
        self.cp_decision_level.save_state()
    }

    fn num_saved(&self) -> u32 {
        // use the CP level as this is what should be externally visible.
        self.cp_decision_level.num_saved()
    }

    fn restore_last(&mut self) {
        // track solver state
        self.cp_decision_level.restore_last();

        // if the solver backtracked past the current LP level, we need to backtrack
        // on it too.
        if self.solver_lvl() < self.lp_level() {
            self.trail.restore_last_with(|lp_event| {
                let _ = self.solver.set_bound(
                    lp_event.var,
                    lp_event.bound,
                    lp_event.old_val,
                    lp_event.old_justification,
                );
            });

            if self
                .pending_bound_constrs_applied_at
                .is_some_and(|lvl| lvl > self.lp_level())
            {
                // We backtracked below the level the pending constraints were applied at, so their bounds are undone while their causes may still be entailed.
                // We reset them all as pending again.
                self.pending_bound_constrs.clear();
                self.pending_bound_constrs.extend(self.bound_constrs.keys());
                self.pending_bound_constrs_applied_at = None;
            }
            debug_assert_eq!(self.solver_lvl(), self.lp_level());
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use std::f64;

    use rand::{
        Rng, SeedableRng,
        rngs::SmallRng,
        seq::{IteratorRandom, SliceRandom},
    };

    use crate::{
        core::{IntCst, state::Cause},
        lang::linear::ScaledVar,
        reasoners::cp::testing::pick_decisions,
    };

    use super::*;

    fn get_nb_x(sparse_proportion: f32, rng: &mut SmallRng, nb_var: usize) -> usize {
        let k = 1.0 / sparse_proportion - 1.0;

        let nb_x_float = (nb_var - 1) as f32 * rng.random::<f32>().powf(k); // we have a f32 in the range [0.0, nb_var - 1)

        1 + nb_x_float as usize
    }

    fn gen_filled_lp_domain(
        nb_var: usize,
        nb_const: usize,
        min: IntCst,
        max: IntCst,
        sparse_proportion: f32,
        seed: u64,
    ) -> (Lp, Domains) {
        let mut lp_reasonner = Lp::default();
        lp_reasonner.activate();

        let mut d = Domains::new();

        let mut rng = SmallRng::seed_from_u64(seed);

        let var_vec: Vec<Var> = (0..nb_var).map(|_| d.new_var(min, max)).collect();

        for _ in 0..nb_const {
            let nb_x = get_nb_x(sparse_proportion, &mut rng, nb_var);

            let x_vec: Vec<&Var> = var_vec.iter().choose_multiple(&mut rng, nb_x);

            let vars: Vec<ScaledVar> = x_vec
                .iter()
                .map(|&&v| ScaledVar {
                    var: v,
                    factor: rng.random_range(min..=max),
                })
                .collect();

            // println!("constraint: {:?}", linear_sum);

            let bound_val = rng.random_range(min..=max);

            let sum = LinSum::new(bound_val, vars);

            let active = d.new_var(-1, 1).geq(0); // Inspired from mul.rs, might need to be changed

            // println!("active var: {:?}, constraint: {:?}", active, sum);

            lp_reasonner.add_cp_linear_leq_constraint(&sum, active, &d);
        }

        (lp_reasonner, d)
    }

    /// Adapted from testing.rs in cp reasoner
    ///
    /// Test that triggers propagation of random decisions and checks the explanations are correct
    ///
    /// IMPORTANT: These tests rely on the `propagate` implementation and are not meaningful if this one is buggy
    /// (but they may show that it is in fact incoherent when called in different contexts)
    fn test_explanations(d: &Domains, lp: &mut Lp) {
        let mut decisions_rng = SmallRng::seed_from_u64(0);
        // function that returns a given number of decisions to be applied later
        // it use the RNG above to drive its random choices
        // new rng for local use
        let mut rng = SmallRng::seed_from_u64(0);

        // println!("\nBounds 1: {:?}", lp.solver.bounds);

        let mut nb_explanation = 0;

        // repeat a large number of random tests
        for _ in 0..100 {
            let mut lp = lp.clone();
            let mut lp_bis = lp.clone();

            if d.variables().all(|v| d.is_bound(v)) {
                println!("Warning: all variables are bound, no tests run");
                return;
            }

            // pick a random set of decisions
            let decisions = pick_decisions(d, 1, 30, &mut decisions_rng);
            // println!("decisions: {decisions:?}");

            // get a copy of the domain on which to apply all decisions
            let mut d = d.clone();
            d.save_state();

            // apply all decisions (note: some may be ignored because they are no-op or contradictions)
            // println!("Decisions: ");
            for dec in decisions {
                let res = d.set(dec, Cause::Decision);
                if res == Ok(true) {
                    // println!("  {dec:?}");
                }
            }
            // propagate
            match lp.propagate(&mut d) {
                Ok(()) => {} // Nothing to do if we do not have a contradiction as the lp reasoner can't infer new lit
                Err(contradiction) => {
                    // propagation failure, check that the contradiction is a valid one
                    let explanation = match contradiction {
                        Contradiction::Explanation(expl) => expl,
                        Contradiction::InvalidUpdate(_) => unreachable!(), // Unreachable branch as our lp never returns InvalidUpdate
                    };

                    nb_explanation += 1;

                    let mut d = d.clone();
                    d.reset();
                    // get the conjunction and shuffle it
                    //note that we do not check minimality here
                    let mut conjuncts = explanation.lits;
                    conjuncts.shuffle(&mut rng);
                    for &conjunct in &conjuncts {
                        d.set(conjunct, Cause::Decision).unwrap();
                    }

                    assert!(
                        lp_bis.propagate(&mut d).is_err(),
                        "explanation: {conjuncts:?} did not trigger an inconsistency\n"
                    );
                }
            }
        }
        println!("{nb_explanation}");
    }

    #[test]
    fn test_propagate_random() {
        let n = 100;
        for seed in 0..n {
            println!("seed: {seed}");
            let (mut lp, d) = gen_filled_lp_domain(30, 30, -100, 100, 0.1, seed);
            test_explanations(&d, &mut lp);
        }
    }

    fn backtracking_single(d: &mut Domains, lp: &mut Lp) {
        let mut decisions_rng = SmallRng::seed_from_u64(0);
        // function that returns a given number of decisions to be applied later
        // it use the RNG above to drive its random choices
        // new rng for local use
        let mut rng = SmallRng::seed_from_u64(0);

        let init_solver = lp.solver.clone();

        // repeat a large number of random tests
        for _ in 0..100 {
            lp.save_state();

            if d.variables().all(|v| d.is_bound(v)) {
                println!("Warning: all variables are bound, no tests run");
                return;
            }

            // pick a random set of decisions
            let decisions = pick_decisions(d, 1, 30, &mut decisions_rng);
            // println!("decisions: {decisions:?}");

            d.save_state();

            // apply all decisions (note: some may be ignored because they are no-op or contradictions)
            // println!("Decisions: ");
            for dec in decisions {
                let res = d.set(dec, Cause::Decision);
                if res == Ok(true) {
                    // println!("  {dec:?}");
                }
            }
            // propagate
            match lp.propagate(d) {
                Ok(()) => {} // Nothing to do if we do not have a contradiction as the lp reasoner can't infer new lit
                Err(contradiction) => {
                    // propagation failure, check that the contradiction is a valid one
                    let explanation = match contradiction {
                        Contradiction::Explanation(expl) => expl,
                        Contradiction::InvalidUpdate(_) => unreachable!(), // Unreachable branch as our lp never returns InvalidUpdate
                    };
                    lp.restore_last();
                    lp.save_state();

                    d.restore_last();
                    d.save_state();

                    // get the conjunction and shuffle it
                    //note that we do not check minimality here
                    let mut conjuncts = explanation.lits;
                    conjuncts.shuffle(&mut rng);
                    for &conjunct in &conjuncts {
                        d.set(conjunct, Cause::Decision).unwrap();
                    }

                    assert!(
                        lp.propagate(d).is_err(),
                        "explanation: {conjuncts:?} did not trigger an inconsistency\n"
                    );
                }
            }

            d.restore_last();
            lp.restore_last();

            assert!(init_solver == lp.solver);
        }
    }

    #[test]
    fn test_backtracking() {
        let n = 100;
        for seed in 0..n {
            println!("seed: {seed}");
            let (mut lp, mut d) = gen_filled_lp_domain(30, 30, -100, 100, 0.1, seed);
            lp.total_allowed_pivots = f64::INFINITY; // unlimited propagation
            backtracking_single(&mut d, &mut lp);
        }
    }

    /// A conflict that a binding took part in must be explained by that binding's literals.
    ///
    /// Reporting an empty explanation instead would read as "infeasible whatever was decided", and
    /// the solver would conclude that the whole problem is unsatisfiable.
    #[test]
    fn test_aux_binding_explains_conflict() {
        let mut d = Domains::new();
        let p = d.new_var(0, 1);
        let q = d.new_var(0, 1);

        let mut lp = Lp::default();
        lp.total_allowed_pivots = f64::INFINITY; // unlimited propagation
        let aux_a: LpVar = lp.create_auxiliary_variable(0, 1);
        let aux_b: LpVar = lp.create_auxiliary_variable(0, 1);
        lp.activate();
        // `-a - b + 1 <= 0`
        lp.add_linear_leq_constraint(vec![(aux_a, -1), (aux_b, -1)], 1, Lit::TRUE, &d);

        lp.add_bound_update_trigger(p.leq(0).into(), BoundRestriction::leq(aux_a, 0), &d);
        lp.add_bound_update_trigger(q.leq(0).into(), BoundRestriction::leq(aux_b, 0), &d);

        d.save_state();
        lp.save_state();
        d.set(p.leq(0), Cause::Decision).unwrap();
        d.set(q.leq(0), Cause::Decision).unwrap();

        let Err(Contradiction::Explanation(expl)) = lp.propagate(&mut d) else {
            panic!("both columns are bound to 0, which `aux_a + aux_b >= 1` forbids");
        };
        assert!(expl.literals().contains(&p.leq(0)), "{:?}", expl.literals());
        assert!(expl.literals().contains(&q.leq(0)), "{:?}", expl.literals());
    }

    /// A bound-update trigger can restrict an LP variable that is mapped to a CP variable
    /// (here through the -1 shortcut of `reify_sum`), without any counterpart event in the main model.
    /// A conflict such a bound takes part in must be explained by the trigger's literals:
    /// the main model alone does not entail the bound.
    #[test]
    fn test_mapped_var_bound_trigger_explains_conflict() {
        let mut d = Domains::new();
        let x = d.new_var(-100, 100);
        let a = d.new_var(-1, 1).geq(0);
        let c = d.new_var(-1, 1).geq(0);

        let mut lp = Lp::default();
        lp.total_allowed_pivots = f64::INFINITY; // unlimited propagation
        lp.activate();

        let lp_x = lp.bind_cp_var(x, &d);
        // s = 97 * x, with s <= 87 when `a`
        lp.add_linear_leq_constraint(vec![(lp_x, 97)], -87, a, &d);
        // -1 shortcut: the bound x >= 34 lands on the mapped variable itself, when `c`
        lp.add_linear_leq_constraint(vec![(lp_x, -1)], 34, c, &d);

        d.save_state();
        lp.save_state();
        d.set(a, Cause::Decision).unwrap();
        d.set(c, Cause::Decision).unwrap();

        let Err(Contradiction::Explanation(expl)) = lp.propagate(&mut d) else {
            panic!("x >= 34 implies 97*x >= 3298, which contradicts 97*x <= 87");
        };
        assert!(expl.literals().contains(&a), "{:?}", expl.literals());
        assert!(expl.literals().contains(&c), "{:?}", expl.literals());
    }

    /// Several bound updates may target the same LP variable, from triggers and from the main-model
    /// synchronization alike. Each restricting update is trailed with the previous value and
    /// justification of the bound, so backtracking must restore the exact prior state.
    #[test]
    fn test_multiple_bound_updates_are_trailed() {
        let mut d = Domains::new();
        let x = d.new_var(-10, 10);
        let a = d.new_var(-1, 1).geq(0);
        let b = d.new_var(-1, 1).geq(0);

        let mut lp = Lp::default();
        lp.activate();
        let lp_x = lp.bind_cp_var(x, &d);

        // two triggers restricting the same upper bound
        lp.add_bound_update_trigger(a.into(), BoundRestriction::leq(lp_x, 5), &d);
        lp.add_bound_update_trigger(b.into(), BoundRestriction::leq(lp_x, 2), &d);

        let init_solver = lp.solver.clone();

        // both triggers apply, the second one restricting the bound further
        d.save_state();
        lp.save_state();
        d.set(a, Cause::Decision).unwrap();
        d.set(b, Cause::Decision).unwrap();
        assert!(lp.propagate(&mut d).is_ok());
        assert_eq!(lp.solver.bounds[lp_x.idx()].upper, 2);

        // backtracking restores the initial bound together with its justification
        lp.restore_last();
        d.restore_last();
        assert!(lp.solver == init_solver);

        // each trigger can now be applied on its own
        d.save_state();
        lp.save_state();
        d.set(a, Cause::Decision).unwrap();
        assert!(lp.propagate(&mut d).is_ok());
        assert_eq!(lp.solver.bounds[lp_x.idx()].upper, 5);
        lp.restore_last();
        d.restore_last();
        assert!(lp.solver == init_solver);

        // a trigger interleaved with a synchronization from the main model
        d.save_state();
        lp.save_state();
        d.set(a, Cause::Decision).unwrap();
        d.set(x.leq(3), Cause::Decision).unwrap();
        assert!(lp.propagate(&mut d).is_ok());
        assert_eq!(lp.solver.bounds[lp_x.idx()].upper, 3);
        lp.restore_last();
        d.restore_last();
        assert!(lp.solver == init_solver);
    }

    /// A bound pushed by a binding does not outlive the cause that justified it: backtracking past that cause releases the bound.
    /// The binding itself of course stays registered, and pushes it again if its cause becomes entailed again.
    #[test]
    fn test_aux_var_bound_is_released_after_backtrack() {
        let mut d = Domains::new();
        let p = d.new_var(0, 1);
        let q = d.new_var(0, 1);

        let mut lp = Lp::default();
        lp.total_allowed_pivots = f64::INFINITY; // unlimited propagation
        let aux_a: LpVar = lp.create_auxiliary_variable(0, 1);
        let aux_b: LpVar = lp.create_auxiliary_variable(0, 1);
        lp.activate();
        // `-a - b + 1 <= 0`
        lp.add_linear_leq_constraint(vec![(aux_a, -1), (aux_b, -1)], 1, Lit::TRUE, &d);

        lp.add_bound_update_trigger(p.leq(0).into(), BoundRestriction::leq(aux_a, 0), &d);
        lp.add_bound_update_trigger(q.leq(0).into(), BoundRestriction::leq(aux_b, 0), &d);

        d.save_state();
        lp.save_state();
        d.set(p.leq(0), Cause::Decision).unwrap();
        d.set(q.leq(0), Cause::Decision).unwrap();
        assert!(lp.propagate(&mut d).is_err());

        lp.restore_last();
        d.restore_last();

        assert!(
            lp.propagate(&mut d).is_ok(),
            "the bindings no longer hold, so the columns should be free again"
        );
    }

    /// A binding registered when its cause is already entailed pushes its bound above the level of that cause.
    /// Backtracking to in-between releases the bound while the cause still holds, so it has to be pushed again, or there will be no watch to fire it.
    #[test]
    fn test_aux_var_bound_is_repushed_after_backtrack() {
        let mut d = Domains::new();
        let p = d.new_var(0, 1);
        let q = d.new_var(0, 1);
        let unrelated = d.new_var(0, 1);

        let mut lp = Lp::default();
        lp.total_allowed_pivots = f64::INFINITY; // unlimited propagation
        let aux_a: LpVar = lp.create_auxiliary_variable(0, 1);
        let aux_b: LpVar = lp.create_auxiliary_variable(0, 1);
        lp.activate();
        // `-a - b + 1 <= 0`
        lp.add_linear_leq_constraint(vec![(aux_a, -1), (aux_b, -1)], 1, Lit::TRUE, &d);

        // level 1: the causes of both bindings become entailed here
        d.save_state();
        lp.save_state();
        d.set(p.leq(0), Cause::Decision).unwrap();
        d.set(q.leq(0), Cause::Decision).unwrap();

        // level 2
        d.save_state();
        lp.save_state();
        d.set(unrelated.leq(0), Cause::Decision).unwrap();

        // level 3: the bindings are only registered now, long after their causes started holding
        d.save_state();
        lp.save_state();

        lp.add_bound_update_trigger(p.leq(0).into(), BoundRestriction::leq(aux_a, 0), &d);
        lp.add_bound_update_trigger(q.leq(0).into(), BoundRestriction::leq(aux_b, 0), &d);

        assert!(lp.propagate(&mut d).is_err());

        lp.restore_last();
        d.restore_last();

        assert!(
            lp.propagate(&mut d).is_err(),
            "`p <= 0` and `q <= 0` still hold at this level, so both columns are still bound to 0"
        );
    }

    /// A binding does not apply while its scope is unknown, and applies once it becomes entailed —
    /// even though the event is then on the scope rather than on the trigger.
    #[test]
    fn test_aux_binding_scope() {
        let mut d = Domains::new();
        let p = d.new_var(0, 1);
        let q = d.new_var(0, 1);
        let scope = d.new_var(0, 1);

        let mut lp = Lp::default();
        lp.total_allowed_pivots = f64::INFINITY; // unlimited propagation
        let aux_a: LpVar = lp.create_auxiliary_variable(0, 1);
        let aux_b: LpVar = lp.create_auxiliary_variable(0, 1);
        lp.activate();
        // `-a - b + 1 <= 0`
        lp.add_linear_leq_constraint(vec![(aux_a, -1), (aux_b, -1)], 1, Lit::TRUE, &d);

        lp.add_bound_update_trigger(p.leq(0).into(), BoundRestriction::leq(aux_a, 0), &d);
        lp.add_bound_update_trigger([scope.geq(1), q.leq(0)].into(), BoundRestriction::leq(aux_b, 0), &d);

        d.save_state();
        lp.save_state();
        d.set(p.leq(0), Cause::Decision).unwrap();
        d.set(q.leq(0), Cause::Decision).unwrap();
        assert!(
            lp.propagate(&mut d).is_ok(),
            "the second binding is out of scope, so its column is still free"
        );

        d.save_state();
        lp.save_state();
        d.set(scope.geq(1), Cause::Decision).unwrap();
        assert!(
            lp.propagate(&mut d).is_err(),
            "entering the scope binds the second column too"
        );
    }

    /// A constraint whose cause holds at the root is applied even though no watch can ever fire for
    /// it, and it survives a backtrack below the level at which it was registered.
    #[test]
    fn test_root_cause_constraint_is_permanent() {
        let mut d = Domains::new();
        let mut lp = Lp::default();
        let aux_a = lp.create_auxiliary_variable(0, 1);
        lp.activate();

        d.save_state();
        lp.save_state();

        // `aux_a <= 0`, registered above the root but justified by `Lit::TRUE`, which can never stop holding.
        // No watch can fire for such a cause, so it is only ever applied by the pending list, without being trailed.
        lp.add_linear_leq_constraint(vec![(aux_a, 1)], 0, Lit::TRUE, &d);
        assert!(lp.propagate(&mut d).is_ok());

        lp.restore_last();
        d.restore_last();

        // `aux_a >= 1`, which only conflicts if the bound above was unaffected by the backtrack
        lp.add_linear_leq_constraint(vec![(aux_a, -1)], 1, Lit::TRUE, &d);
        assert!(
            lp.propagate(&mut d).is_err(),
            "a bound justified at the root must not be undone by a backtrack"
        );
    }
}
