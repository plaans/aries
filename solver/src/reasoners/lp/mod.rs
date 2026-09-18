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

[ref-doc]: https://link.springer.com/chapter/10.1007/11817963_11
*/

mod explanation_lang;
mod solver;

#[cfg(feature = "lp_log")]
mod log;

use std::collections::HashMap;

use aries_env_param::EnvParam;
#[allow(unused_imports)]
use itertools::Itertools;
use solver::Solver;

use aries_lp::{Bound, Error, Variable};

use crate::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail},
    collections::ref_store::RefMap,
    core::{
        Lit, LongCst, Var, cst_int_to_long,
        literals::Watches,
        state::{Domains, DomainsSnapshot, Event, Explanation, InferenceCause},
    },
    lang::linear::{LinSum, ScaledVar},
    reasoners::{Contradiction, ReasonerId, Theory},
};

/// Contains all the options available for the Lp reasonner
///
/// It can be passed when creating the reasonner or modified through following methods:
/// [`Lp::activate`], [`Lp::deactivate`], [`Lp::deactivate_propagation`], [`Lp::activate_refined_explanation`] and [`Lp::deactivate_refined_explanation`]
#[derive(Debug, Clone, Copy)]
pub struct LpOptions {
    /// Used to activate/deactivate the propagation of the reasonner
    ///
    /// Can be controlled with the following methods: [`Lp::activate`], [`Lp::deactivate`] and [`Lp::deactivate_propagation`]
    propagation_active: bool,
    /// Used to enable/disable the reasonner
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

/// Used to enable/disable the lp reasonner
///
/// Can be set either from the env variable **`ARIES_LP_ENABLE`**
/// or within the code with: `LP_ENABLE.set(true/false)`
pub static LP_ENABLE: EnvParam<bool> = EnvParam::new("ARIES_LP_ENABLE", "false");

#[derive(Debug, Clone, Copy)]
struct BoundConstraint {
    var: Variable,
    bound: Bound,
    val: LongCst,
}

/// Store all the necessary information for backtracking after modifying a bound
///
/// Only the old value and activation literals are necessary as they will overwrite the current value
#[derive(Clone)]
struct LpEvent {
    /// variable affected by the bound change
    var: Variable,
    /// bound modified
    bound: Bound,
    /// Old value for the bound that needs to overwrite the new one when backtracking
    old_val: LongCst,
    /// Activation literal associated with this bound and value
    old_lit: Lit,
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

/// Struct that implements the [`Theory`] trait (reasoner).
///
/// It encapsulates all the necessary information to run the lp solver on the posted constraints.
///
/// The propagation can be dynamically activated / deactivated through the following methods: [`Lp::activate()`] and [`Lp::deactivate()`].
/// This can be useful to avoid an overhead of the lp reasonner over the others if its not relevant.
#[derive(Clone)]
pub struct Lp {
    id: ReasonerId,
    /// Encapsulates both float and integer versions of our constraints and an instance of the aries-lp solver
    solver: Solver,
    /// Associates each bound constraint with its activation lit
    bound_cons_lit_vec: Vec<(BoundConstraint, Lit)>,
    /// Associates linear sums with its corresponding variable in the aries-lp solver
    ///
    /// It is used to avoid duplicate variables that should be the same
    memory_s: HashMap<Vec<ScaledVar>, Variable>,
    /// Maps var from aries solver with their coresponding variable in aries-lp (if they appear in the post constraints)
    memory_x: RefMap<Var, Variable>,
    model_events: ObsTrailCursor<Event>,
    /// The watcher corresponds to an index in bound_cons_lit_vec
    watches: Watches<usize>,
    /// History of changes made to the LP with all information necessary to undo them.
    trail: Trail<LpEvent>,
    stats: Stats,
    /// Contains all the customizable options for the reasonner
    ///
    /// Check [`LpOptions`] for more details
    options: LpOptions,
    /// Used to log the initial problem
    ///
    /// It supposes that no additonal constraint is added after the first propagation
    #[cfg(feature = "lp_log")]
    is_first_propagate: bool,
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

            bound_cons_lit_vec: Vec::new(),

            memory_s: HashMap::new(),
            memory_x: RefMap::default(),

            model_events: ObsTrailCursor::new(),
            watches: Default::default(),
            trail: Default::default(),

            stats: Stats::new(),

            options,
            #[cfg(feature = "lp_log")]
            is_first_propagate: true,
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

    /// Returns a linear sum which is the opposite in terms of coefficient that the one given
    ///
    /// We use it to detect that 2 constraints could use the same s variable in aries-lp
    fn get_opposite_linear_sum(linear_sum: &[ScaledVar]) -> Vec<ScaledVar> {
        let mut opp = Vec::new();
        for &svar in linear_sum {
            opp.push(ScaledVar {
                var: svar.var,
                factor: -svar.factor,
            });
        }
        opp
    }

    /// Add an x variable which is a variable directly mapped with a var in aries solver
    ///
    /// Check the reference paper for more details: [A Fast Linear-Arithmetic Solver for DPLL(T)][ref-doc]
    fn add_x_var(&mut self, x: Var, doms: &Domains) {
        let var = self.solver.create_variable(
            cst_int_to_long(doms.lb(x)),
            cst_int_to_long(doms.ub(x)),
            &mut self.stats,
        );

        self.memory_x.insert(x, var);
        self.solver.map_lp_to_aries.insert(var.idx(), x);
    }

    /// Add an s variable, it corresponds to a linear constraint in aries solver
    ///
    /// They are artificial variables used to be able to activate/deactivate linear constraints just by setting a bound to it.
    /// Check the reference paper for more details: [A Fast Linear-Arithmetic Solver for DPLL(T)][ref-doc]
    fn add_s_var(&mut self, linear_sum: &[ScaledVar], doms: &Domains) -> Variable {
        for &svar in linear_sum {
            if !self.memory_x.contains(svar.var) {
                self.add_x_var(svar.var, doms);
            }
        }

        // If we depend on only one x variable with a factor 1, no need to create a s var
        if linear_sum.len() == 1 && linear_sum[0].factor == 1 {
            return *self.memory_x.get(linear_sum[0].var).unwrap();
        }

        let mut constraint = vec![];

        let mut lb: LongCst = 0;
        let mut ub: LongCst = 0;

        for &svar in linear_sum {
            ub = ub.saturating_add(svar.upper_bound_long(doms));
            lb = lb.saturating_add(svar.lower_bound_long(doms));

            let var = *self.memory_x.get(svar.var).unwrap();
            constraint.push((var, svar.factor));
        }

        let s = self.solver.create_variable(lb, ub, &mut self.stats);

        constraint.push((s, -1));

        // We force s to be equal to our linear sum
        self.solver.add_constraint(constraint);
        self.stats.num_constraints += 1;

        self.memory_s.insert(linear_sum.to_vec(), s);

        s
    }

    /// Post a LinearLeq constraint of the form `sum <= 0`.
    /// The constant term is included in the sum.
    ///
    /// `active` is the activation [`Lit`], the constraint is only active when it is evaluated to `true`
    /// We assume that the active literal is always present, it is the responsability of the caller to ensure it:
    /// `doms.presence(active) == Lit::TRUE`
    pub fn add_linear_leq_constraint(&mut self, sum: &LinSum, active: Lit, doms: &Domains) {
        if !self.options.enable {
            return;
        }

        // Check that the given constraint is always present (not optionnal)
        assert!(doms.presence(active) == Lit::TRUE);

        let bound_val = cst_int_to_long(-sum.constant());

        let elements = sum.terms_slice().to_vec();

        let opp_lin_sum = Lp::get_opposite_linear_sum(&elements);

        let bound_cons: BoundConstraint;

        if self.memory_s.contains_key(&elements) {
            let &s = self.memory_s.get(&elements).unwrap();
            bound_cons = BoundConstraint {
                var: s,
                bound: Bound::Upper,
                val: bound_val,
            };
        } else if self.memory_s.contains_key(&opp_lin_sum) {
            // If an s variable already exists for the opposite of our linear sum, we can use the same by inverting our constraint

            let &s = self.memory_s.get(&opp_lin_sum).unwrap();
            bound_cons = BoundConstraint {
                var: s,
                bound: Bound::Lower,
                val: -bound_val,
            };
        } else {
            let s = self.add_s_var(&elements, doms);

            bound_cons = BoundConstraint {
                var: s,
                bound: Bound::Upper,
                val: bound_val,
            };
        }

        let index = self.bound_cons_lit_vec.len();
        self.watches.add_watch(index, active);

        // We memorize our constraint and its active lit to be able to access it during propagation
        self.bound_cons_lit_vec.push((bound_cons, active));
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

        #[cfg(feature = "lp_log")]
        {
            if self.is_first_propagate {
                self.is_first_propagate = false;
                self.solver.logger.set_problem(self.solver.problem.clone()); // We save the initial state of our problem
            }
        }

        self.stats.num_propagate += 1;

        // We process all the newly inferred literals since last propagation
        while let Some(&event) = self.model_events.pop(domains.trail()) {
            let lit = event.new_literal();

            // println!("Lit: {:?}", lit);

            // We first set the bounds associated with the active lit triggered by the newly inferred lit
            let watchers: Vec<usize> = self.watches.watches_on(lit).collect();
            for watcher in watchers {
                let (bound_cons, active_lit) = self.bound_cons_lit_vec[watcher];

                let res = self.solver.set_bound_restrict(
                    bound_cons.var,
                    bound_cons.bound,
                    bound_cons.val,
                    active_lit,
                    &mut self.trail,
                );

                self.explain_set_bound(&res, bound_cons.var)?;
            }

            let var = event.affected_bound.variable();

            // We update the bound of the corresponding variable of the lit in the lp solver (if there is one)
            if let Some(&x_var) = self.memory_x.get(var) {
                let res =
                    // if we have is_plus, the constraint is of the form x <= b therefore it's an upper bound
                    if event.affected_bound.is_plus() {
                        self.solver
                            .set_bound_restrict(x_var, Bound::Upper, cst_int_to_long(event.new_upper_bound), lit, &mut self.trail)
                    } else {
                        self.solver
                            .set_bound_restrict(x_var, Bound::Lower, cst_int_to_long(-event.new_upper_bound), lit, &mut self.trail)
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

    // Should not be called as this reasonner never infers new lit, it only gives contradictions
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
            let _ = self
                .solver
                .set_bound(lp_event.var, lp_event.bound, lp_event.old_val, lp_event.old_lit);
        });
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
        reasoners::cp::testing::pick_decisions,
    };

    use super::*;

    #[test]
    fn opposite_linear_sum() {
        {
            let v1 = Var::from_u32(1);
            let v2 = Var::from_u32(2);
            let v3 = Var::from_u32(3);
            let lin_sum = vec![
                ScaledVar { var: v1, factor: 4 },
                ScaledVar { var: v3, factor: -1 },
                ScaledVar { var: v2, factor: 6 },
            ];

            assert_eq!(
                Lp::get_opposite_linear_sum(&lin_sum),
                vec![
                    ScaledVar { var: v1, factor: -4 },
                    ScaledVar { var: v3, factor: 1 },
                    ScaledVar { var: v2, factor: -6 },
                ]
            )
        }

        {
            let lin_sum = vec![];

            assert_eq!(Lp::get_opposite_linear_sum(&lin_sum), vec![])
        }
    }

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

            lp_reasonner.add_linear_leq_constraint(&sum, active, &d);
        }

        (lp_reasonner, d)
    }

    /// Adapted from testing.rs in cp reasonner
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
                Ok(()) => {} // Nothing to do if we do not have a contradiction as the lp reasonner can't infer new lit
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
                Ok(()) => {} // Nothing to do if we do not have a contradiction as the lp reasonner can't infer new lit
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
}
