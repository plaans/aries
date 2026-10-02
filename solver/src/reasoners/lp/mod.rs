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

> **Important:** This reasoner is **disabled by default**.
> To enable it, set the environment variable **`ARIES_LP_ENABLE=true`** or overwrite the EnvParam [`LP_ENABLE`] using [`EnvParam::set()`].

[ref-doc]: https://doi.org/10.1007/11817963_11
*/

mod explanation_utils;
mod solver;

use std::collections::HashMap;

use aries_env_param::EnvParam;
#[allow(unused_imports)]
use itertools::Itertools;
use solver::{BoundJustification, Solver};

use aries_lp::{Bound, Error, Variable};

use crate::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail},
    collections::ref_store::RefMap,
    core::{
        IntCst, Lit, LongCst, Var, cst_int_to_long,
        literals::Watches,
        state::{Domains, DomainsSnapshot, Event, Explanation, InferenceCause},
    },
    lang::linear::LinSum,
    prelude::Conjunction,
    reasoners::{Contradiction, ReasonerId, Theory, lp::solver::BoundCause},
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
    propagation_active: bool,
    /// Used to enable/disable the reasoner
    ///
    /// Can be controlled trough the following environment variable: ARIES_LP_ENABLE
    /// or after the creation with [`Lp::activate`] and [`Lp::deactivate`]
    enable: bool,
    /// If true, refined explanation are used with minimization based on [`crate::reasoners::cp::linear`]
    ///
    ///Can be controlled with the following methods: [`Lp::activate_refined_explanation`], [`Lp::deactivate_refined_explanation`]
    is_explanation_refined: bool,
}

impl LpOptions {
    fn new(propagation_active: bool, enable: bool, is_explanation_refined: bool) -> Self {
        LpOptions {
            propagation_active,
            enable,
            is_explanation_refined,
        }
    }
}

impl Default for LpOptions {
    fn default() -> Self {
        LpOptions::new(true, LP_ENABLE.get(), true)
    }
}

/// Used to enable/disable the lp reasoner
///
/// Can be set either from the env variable **`ARIES_LP_ENABLE`**
/// or within the code with: `LP_ENABLE.set(true/false)`
pub static LP_ENABLE: EnvParam<bool> = EnvParam::new("ARIES_LP_ENABLE", "false");

#[derive(Debug, Clone, Copy)]
pub struct BoundConstraint {
    var: Variable,
    bound: Bound,
    val: LongCst,
}

impl BoundConstraint {
    pub fn leq(var: Variable, ub: LongCst) -> Self {
        Self {
            var,
            bound: Bound::Upper,
            val: ub,
        }
    }
    pub fn geq(var: Variable, lb: LongCst) -> Self {
        Self {
            var,
            bound: Bound::Lower,
            val: lb,
        }
    }
}

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
    /// Number of propagations where no contradaction was detected, used to determine the number of contradiction generated
    num_ok_propagate: usize,
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
    /// Number of valid certificates generated using float calculations for verification
    num_val_certif_float: usize,
    /// Number of overflows detected during the certificate verification
    num_overflow: usize,
}

impl Stats {
    fn new() -> Self {
        Self {
            num_ok_propagate: 0,
            num_propagate: 0,

            num_constraints: 0,
            num_variables: 0,

            num_certif: 0,
            num_val_certif: 0,
            num_val_certif_float: 0,
            num_overflow: 0,
        }
    }
}

pub type LpVar = aries_lp::Variable;
pub type LpSum = Vec<(LpVar, IntCst)>;
pub enum BoundRestriction {
    Ub(IntCst),
    Lb(IntCst),
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
    /// Associates each bound constraint with the cause that triggers it:
    /// the constraint is applied to the LP as soon as all the literals of its cause (scope and trigger) are entailed.
    ///
    /// Holds both the activation of the linear constraints posted to the reasoner
    /// (whose cause is their activation lit) and the (partial) bindings of auxiliary variables to the main model
    /// (see [`Lp::half_bind_aux_var_lower_bound`]).
    bound_constrs_lit_vec: Vec<(BoundConstraint, BoundCause)>,
    /// Indices in `bound_constrs_lit_vec` of the constraints whose cause was already entailed when
    /// they were registered, and that are thus waiting to be applied at the next propagation.
    pending_bound_constrs: Vec<usize>,
    /// Decision level at which the pending constraints were last applied. No watch can trigger them
    /// again, so backtracking below it makes them all pending anew.
    pending_bound_constrs_applied_at: DecLvl,
    /// Associates linear sums with its corresponding variable in the aries-lp solver.
    /// The sums are simplified, i.e. their terms are sorted those sharing the same var are merged.
    ///
    /// Used to avoid duplicate variables for the same sum (or its opposite).
    memory_s: HashMap<LpSum, Variable>,
    /// Maps var from aries solver with their coresponding variable in minilp (if they appear in the post constraints)
    memory_x_main: RefMap<Var, Variable>,
    model_events: ObsTrailCursor<Event>,
    /// The watcher corresponds to an index in `bound_constrs_lit_vec`
    watches: Watches<usize>,
    /// History of changes made to the LP with all information necessary to undo them.
    trail: Trail<LpEvent>,
    stats: Stats,
    /// Contains all the customizable options for the reasoner
    ///
    /// Check [`LpOptions`] for more details
    options: LpOptions,
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

            bound_constrs_lit_vec: Vec::new(),
            pending_bound_constrs: Vec::new(),
            pending_bound_constrs_applied_at: DecLvl::ROOT,

            memory_s: HashMap::new(),
            memory_x_main: RefMap::default(),

            model_events: ObsTrailCursor::new(),
            watches: Default::default(),
            trail: Default::default(),

            stats: Stats::new(),

            options,
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

    fn reify_sum(&mut self, sum: LpSum) -> (LpVar, bool) {
        // if the sum is exactly one variable, with factor +/-1, no need for a new variable to reify.
        if let Some([(var, 1)]) = sum.as_array() {
            return (*var, false);
        }
        if let Some([(var, -1)]) = sum.as_array() {
            return (*var, true);
        }

        if let Some(reif) = self.memory_s.get(&sum) {
            return (*reif, false);
        }
        let minus_sum: LpSum = sum.iter().map(|&(var, factor)| (var, -factor)).collect();
        if let Some(reif) = self.memory_s.get(&minus_sum) {
            return (*reif, true);
        }

        let mut constraint = vec![];

        let mut lb: LongCst = 0;
        let mut ub: LongCst = 0;

        for &(var, factor) in &sum {
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
        constraint.push((s, -1));

        // We force s to be equal to our linear sum
        self.solver.add_constraint(constraint);
        self.stats.num_constraints += 1;

        self.memory_s.insert(sum, s);

        (s, false)
    }

    /// Post a LinearLeq constraint of the form `sum <= 0`.
    /// The constant term is included in the sum.
    ///
    /// `active` is the activation [`Lit`], the constraint is only active when it is evaluated to `true`
    /// We assume that the active literal is always present, it is the responsibility of the caller to ensure it:
    /// `doms.presence(active) == Lit::TRUE`
    pub fn add_linear_leq_constraint_simple(&mut self, sum: &LinSum, active: Lit, doms: &Domains) {
        let sum_cst = sum.constant();
        let sum_terms = sum
            .terms_slice()
            .iter()
            .map(|&svar| (self.bind_cp_var(svar.var, doms), svar.factor))
            .collect::<Vec<_>>();
        self.add_linear_leq_constraint(sum_terms, sum_cst, active, doms)
    }

    pub fn add_linear_leq_constraint(&mut self, sum_terms: LpSum, sum_cst: IntCst, active: Lit, doms: &Domains) {
        if !self.options.enable {
            return;
        }

        // Check that the given constraint is always present (not optional)
        assert!(doms.presence(active) == Lit::TRUE);

        let bound_val = cst_int_to_long(-sum_cst);

        let (reif, signed) = self.reify_sum(sum_terms);
        let bound_constr = if signed {
            // sum_terms = -reif
            // sum_terms  <= bound_val
            // -reif <= bound_val
            // reif >= -bound_val
            BoundConstraint::geq(reif, -bound_val)
        } else {
            // sum_terms = reif
            // reif <= bound_val
            BoundConstraint::leq(reif, bound_val)
        };

        // this constraint should be activated when `active is entailed and present
        let trigger = Conjunction::from([active, doms.presence(active)]);
        self.add_bound_update_trigger(trigger, bound_constr, doms);
    }

    pub fn add_bound_update_trigger(&mut self, trigger: Conjunction, bound_update: BoundConstraint, domains: &Domains) {
        // TODO: this function supports the API we want but its implementation delegates to the previous one, with obvious limitations
        let cause = match trigger.literals() {
            [] => BoundCause::None,
            [l] => BoundCause::Some {
                scope: Lit::TRUE,
                trigger: *l,
            },
            [l1, l2] => BoundCause::Some {
                scope: *l1,
                trigger: *l2,
            },
            _ => unimplemented!("Still unsupported (will do it later to phase the transition)"),
        };
        self.register_bound_constr(bound_update, cause, domains);
    }

    /// Registers a bound constraint, to be applied to the LP as soon as all the literals of `cause` are entailed in the main model.
    ///
    /// If they are *already* entailed, the constraint is scheduled to be applied at the next call to
    /// [`Theory::propagate`]: watches only fire on future events, so an already entailed cause
    /// (in particular an unconditional one) would otherwise never trigger.
    fn register_bound_constr(&mut self, bound_constr: BoundConstraint, cause: BoundCause, domains: &Domains) {
        let index = self.bound_constrs_lit_vec.len();
        self.bound_constrs_lit_vec.push((bound_constr, cause));

        let mut entailed = true;
        if let BoundCause::Some { scope, trigger } = cause {
            if !scope.tautological() {
                self.watches.add_watch(index, scope);
                entailed &= domains.entails(scope);
            }
            if !trigger.tautological() {
                self.watches.add_watch(index, trigger);
                entailed &= domains.entails(trigger);
            }
        }

        if entailed {
            self.pending_bound_constrs.push(index);
        }
    }

    /// Applies the bound constraints whose cause was already entailed when they were registered.
    fn apply_pending_bound_constrs(&mut self, domains: &Domains) -> Result<(), Contradiction> {
        if !self.pending_bound_constrs.is_empty() {
            debug_assert!(self.pending_bound_constrs_applied_at <= self.current_decision_level());
            self.pending_bound_constrs_applied_at = self.current_decision_level();
        }

        while let Some(index) = self.pending_bound_constrs.pop() {
            let (bound_constr, cause) = self.bound_constrs_lit_vec[index];
            let (scope, trigger) = match cause {
                BoundCause::None => (Lit::TRUE, Lit::TRUE),
                BoundCause::Some { scope, trigger } => (scope, trigger),
            };

            if !domains.entails(scope) || !domains.entails(trigger) {
                // No longer entailed (we backtracked since the registration).
                // The watches will trigger the constraint again if its cause becomes entailed anew.
                continue;
            }

            let res =
                if domains.entailing_level(scope) == DecLvl::ROOT && domains.entailing_level(trigger) == DecLvl::ROOT {
                    // A cause entailed at the root can never be undone,
                    // so the bound is set permanently instead of being trailed and lost on the first backtrack below the current level.
                    self.solver
                        .set_bound_restrict_permanent(bound_constr.var, bound_constr.bound, bound_constr.val)
                } else {
                    self.solver.set_bound_restrict(
                        bound_constr.var,
                        bound_constr.bound,
                        bound_constr.val,
                        BoundJustification::Trigger(cause),
                        &mut self.trail,
                    )
                };

            self.explain_set_bound(&res, bound_constr.var)?;
        }
        Ok(())
    }

    /// Takes the result of a call to [Solver::set_bound_restrict] and returns either `Ok` or a `Contradiction` if infeasibility was detected
    fn explain_set_bound<T>(&mut self, res: &Result<T, Error>, var: Variable) -> Result<(), Contradiction> {
        match res {
            Err(Error::InfeasibleTrivial) => {
                let explanation = self.solver.explain_infeasible_var(var);
                Err(Contradiction::Explanation(explanation))
            }
            Err(Error::InfeasibleWithCertificate(_)) => {
                unreachable!("Setting a bound should not generate a certificate")
            }
            _ => Ok(()),
        }
    }

    /// Takes the result of a call to [Solver::check_feasibility] and returns either `Ok` or a `Contradiction` if infeasibility was detected
    fn explain_check_feas(&mut self, res: Result<(), Error>, domains: &Domains) -> Result<(), Contradiction> {
        match res {
            Err(Error::InfeasibleWithCertificate(cert)) => {
                self.stats.num_certif += 1;
                if self.solver.problem.is_certificate_valid(&cert) {
                    self.stats.num_val_certif_float += 1;
                }
                match self.solver.check_certificate(&cert, domains, &mut self.stats) {
                    Some(explanation) => {
                        self.stats.num_val_certif += 1;
                        Err(Contradiction::Explanation(explanation))
                    }
                    None => {
                        // println!("CHECK FEAS");
                        // let filtered_cert = cert.iter().enumerate().filter(|(_, v)| **v != 0.0).collect_vec();
                        // println!("Invalid certificate: {:?}", filtered_cert);
                        // // if filtered_cert.len() == 1 {
                        // //     println!("Constraint: {:?}", self.solver.constraints[filtered_cert[0].0]);
                        // // }
                        // println!();
                        Ok(())
                    }
                }
            }
            _ => Ok(()),
        }
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

        self.stats.num_propagate += 1;

        // Constraints registered with an already entailed cause are not triggered by any watch
        self.apply_pending_bound_constrs(domains)?;

        // We process all the newly inferred literals since last propagation
        while let Some(&event) = self.model_events.pop(domains.trail()) {
            let lit = event.new_literal();

            // println!("Lit: {:?}", lit);

            // We first set the bounds associated with the active lit triggered by the newly inferred lit
            let watchers: Vec<usize> = self.watches.watches_on(lit).collect();
            for watcher in watchers {
                let (bound_constr, cause) = self.bound_constrs_lit_vec[watcher];

                // A cause with two literals is watched on both of them:
                // the one that did not trigger this watch still has to be entailed for the bound to hold.
                if let BoundCause::Some { scope, trigger } = cause
                    && (!domains.entails(scope) || !domains.entails(trigger))
                {
                    continue;
                }

                let res = self.solver.set_bound_restrict(
                    bound_constr.var,
                    bound_constr.bound,
                    bound_constr.val,
                    BoundJustification::Trigger(cause),
                    &mut self.trail,
                );

                self.explain_set_bound(&res, bound_constr.var)?;
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
                            .set_bound_restrict(x_var, Bound::Upper, cst_int_to_long(event.new_upper_bound), justification, &mut self.trail)
                    } else {
                        self.solver
                            .set_bound_restrict(x_var, Bound::Lower, cst_int_to_long(-event.new_upper_bound), justification, &mut self.trail)
                    };

                self.explain_set_bound(&res, x_var)?;
            }
        }

        // After updating all the bounds, we check that our lp solver is still in a feasible state
        let res = self.solver.check_feasibility();
        self.explain_check_feas(res, domains)?;

        self.stats.num_ok_propagate += 1;

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
            println!(
                "# contradictions: {}",
                self.stats.num_propagate - self.stats.num_ok_propagate
            );
            println!("# constraints: {}", self.stats.num_constraints);
            println!("# variables: {}", self.stats.num_variables);
            println!(
                "# certificates: {}, valid: {}, overflow: {}",
                self.stats.num_certif, self.stats.num_val_certif, self.stats.num_overflow
            );
            println!("# valid float certificates: {}", self.stats.num_val_certif_float);
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
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.trail.restore_last_with(|lp_event| {
            let _ = self.solver.set_bound(
                lp_event.var,
                lp_event.bound,
                lp_event.old_val,
                lp_event.old_justification,
            );
        });

        if self.current_decision_level() < self.pending_bound_constrs_applied_at {
            // We backtracked below the level the pending constraints were applied at, so their bounds are undone while their causes may still be entailed.
            // We reset them all as pending again.
            self.pending_bound_constrs.clear();
            self.pending_bound_constrs.extend(0..self.bound_constrs_lit_vec.len());
            self.pending_bound_constrs_applied_at = DecLvl::ROOT;
        }
    }
}

#[cfg(test)]
mod tests {
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

            lp_reasonner.add_linear_leq_constraint_simple(&sum, active, &d);
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
        let aux_a: LpVar = lp.create_auxiliary_variable(0, 1);
        let aux_b: LpVar = lp.create_auxiliary_variable(0, 1);
        lp.activate();
        // `-a - b + 1 <= 0`
        lp.add_linear_leq_constraint(vec![(aux_a, -1), (aux_b, -1)], 1, Lit::TRUE, &d);

        lp.add_bound_update_trigger(p.leq(0).into(), BoundConstraint::leq(aux_a, 0), &d);
        lp.add_bound_update_trigger(q.leq(0).into(), BoundConstraint::leq(aux_b, 0), &d);

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
        lp.add_bound_update_trigger(a.into(), BoundConstraint::leq(lp_x, 5), &d);
        lp.add_bound_update_trigger(b.into(), BoundConstraint::leq(lp_x, 2), &d);

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
        let aux_a: LpVar = lp.create_auxiliary_variable(0, 1);
        let aux_b: LpVar = lp.create_auxiliary_variable(0, 1);
        lp.activate();
        // `-a - b + 1 <= 0`
        lp.add_linear_leq_constraint(vec![(aux_a, -1), (aux_b, -1)], 1, Lit::TRUE, &d);

        lp.add_bound_update_trigger(p.leq(0).into(), BoundConstraint::leq(aux_a, 0), &d);
        lp.add_bound_update_trigger(q.leq(0).into(), BoundConstraint::leq(aux_b, 0), &d);

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

        lp.add_bound_update_trigger(p.leq(0).into(), BoundConstraint::leq(aux_a, 0), &d);
        lp.add_bound_update_trigger(q.leq(0).into(), BoundConstraint::leq(aux_b, 0), &d);

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
        let aux_a: LpVar = lp.create_auxiliary_variable(0, 1);
        let aux_b: LpVar = lp.create_auxiliary_variable(0, 1);
        lp.activate();
        // `-a - b + 1 <= 0`
        lp.add_linear_leq_constraint(vec![(aux_a, -1), (aux_b, -1)], 1, Lit::TRUE, &d);

        lp.add_bound_update_trigger(p.leq(0).into(), BoundConstraint::leq(aux_a, 0), &d);
        lp.add_bound_update_trigger([scope.geq(1), q.leq(0)].into(), BoundConstraint::leq(aux_b, 0), &d);

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
