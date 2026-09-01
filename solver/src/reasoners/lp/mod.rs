mod solver;

#[cfg(feature = "lp_log")]
mod log;

use std::collections::HashMap;

use aries_env_param::EnvParam;
#[allow(unused_imports)]
use itertools::Itertools;
use solver::Solver;

use minilp::{Bound, Error, Variable};

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

pub static LP_ENABLE: EnvParam<bool> = EnvParam::new("ARIES_LP_ENABLE", "true");

#[derive(Debug, Clone, Copy)]
struct BoundConstraint {
    var: Variable,
    bound: Bound,
    val: LongCst,
}

// Store all the necessary information for backtracking after modifying a bound
#[derive(Clone)]
struct LpEvent {
    var: Variable,
    bound: Bound,
    old_val: LongCst,
    old_lit: Lit,
}

#[derive(Clone)]
struct Stats {
    // Number of propagations where no contradaction was detected
    num_ok_propagate: usize,
    num_propagate: usize,

    num_constraints: usize,
    num_variables: usize,

    num_certif: usize,
    num_val_certif: usize,
    num_val_certif_float: usize,
    num_overflow: usize,

    history_certif: Vec<f32>,
    last_num_val_certif: usize,
}

impl Stats {
    const NUM_CERT_PROPORTION: usize = 100;

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

            history_certif: Vec::new(),
            last_num_val_certif: 0,
        }
    }

    fn update_history_certif(&mut self) {
        if self.num_certif.is_multiple_of(Stats::NUM_CERT_PROPORTION) {
            let proportion =
                (self.num_val_certif - self.last_num_val_certif) as f32 / Stats::NUM_CERT_PROPORTION as f32;
            self.last_num_val_certif = self.num_val_certif;
            self.history_certif.push(proportion);
        }
    }
}

#[derive(Clone)]
pub struct Lp {
    id: ReasonerId,
    /// Encapsulates both float and integer versions of our constraints and an instance of the minilp solver
    solver: Solver,
    /// Associates each bound constraint with its activation lit
    bound_cons_lit_vec: Vec<(BoundConstraint, Lit)>,
    /// Associates linear sums with its corresponding variable in the minilp solver
    ///
    /// It is used to avoid duplicate variables that should be the same
    memory_s: HashMap<Vec<ScaledVar>, Variable>,
    /// Maps var from aries solver with their coresponding variable in minilp (if they appear in the post constraints)
    memory_x: RefMap<Var, Variable>,
    model_events: ObsTrailCursor<Event>,
    /// The watcher corresponds to an index in bound_cons_lit_vec
    watches: Watches<usize>,
    /// History of changes made to the LP with all information necessary to undo them.
    trail: Trail<LpEvent>,
    stats: Stats,
    /// Used to activate/deactivate the propagation of the reasonner
    ///
    /// It can be controlled with activate and deactivate methods
    active: bool,
    /// Used to enable/disable the reasonner
    ///
    /// Can be controlled trough the following environment variable: ARIES_LP_ENABLE
    enable: bool,
    /// Used to log the initial problem
    ///
    /// It supposes that no additonal constraint is added after the first propagation
    #[cfg(feature = "lp_log")]
    is_first_propagate: bool,
}

impl Default for Lp {
    fn default() -> Self {
        Self::new()
    }
}

impl Lp {
    pub fn new() -> Self {
        Self {
            id: ReasonerId::Cp,
            solver: Solver::new(),

            bound_cons_lit_vec: Vec::new(),

            memory_s: HashMap::new(),
            memory_x: RefMap::default(),

            model_events: ObsTrailCursor::new(),
            watches: Default::default(),
            trail: Default::default(),

            stats: Stats::new(),

            enable: LP_ENABLE.get(),
            active: true,
            #[cfg(feature = "lp_log")]
            is_first_propagate: true,
        }
    }
    /// Activate propagation of the LP
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate propagation of the LP
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Return a linear sum wich is the opposite in terms of coefficient that the one given
    ///
    /// We use it to detect that 2 constraints could use the same s variable in minilp
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
    fn add_x_var(&mut self, x: Var, doms: &Domains) {
        let var = self.solver.create_variable(
            cst_int_to_long(doms.lb(x)),
            cst_int_to_long(doms.ub(x)),
            &mut self.stats,
        );

        self.memory_x.insert(x, var);
    }

    /// Add an s variable, it corresponds to a linear constraint in aries solver
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

    /// Adds a linear inequality constraint that `sum <= 0`.
    /// We assume that the active literal is always present
    pub fn add_linear_leq_constraint(&mut self, sum: &LinSum, active: Lit, doms: &Domains) {
        if !self.enable {
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

    /// Takes the result of a call to solver.set_bound_result and returns either Ok or a Contradiction if infeasibilty was detected
    fn explain_set_bound<T>(&mut self, res: &Result<T, Error>, var: Variable) -> Result<(), Contradiction> {
        match res {
            Err(Error::Infeasible) => {
                let explanation = self.solver.explain_infeasible_var(var);
                Err(Contradiction::Explanation(explanation))
            }
            Err(Error::InfeasibleWithCertificate(_)) => {
                unreachable!("Setting a bound should not generate a certificate")
            }
            _ => Ok(()),
        }
    }

    /// Takes the result of a call to solver.check_feasibility and returns either Ok or a Contradiction if infeasibilty was detected
    fn explain_check_feas(&mut self, res: Result<(), Error>) -> Result<(), Contradiction> {
        match res {
            Err(Error::InfeasibleWithCertificate(cert)) => {
                self.stats.num_certif += 1;
                self.stats.update_history_certif();
                if self.solver.problem.is_certificate_valid(&cert) {
                    self.stats.num_val_certif_float += 1;
                }
                match self.solver.check_certificate(&cert, &mut self.stats) {
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
        if !self.active || !self.enable {
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
        self.explain_check_feas(res)?;

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
        if self.enable {
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
            // println!("History proportion valid cert: {:?}", self.stats.history_certif);
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
        core::{INT_CST_MAX, INT_CST_MIN, IntCst, state::Cause},
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

    impl Lp {
        fn get_validity_certificates(&mut self) -> Option<(bool, bool)> {
            let bound_cons_lit_vec = self.bound_cons_lit_vec.clone();

            for (bound_cons, _) in bound_cons_lit_vec {
                let res_set_bound = self
                    .solver
                    .set_bound(bound_cons.var, bound_cons.bound, bound_cons.val, Lit::TRUE);

                let res_check_feas = self.solver.check_feasibility();

                if res_set_bound.is_err() || res_check_feas.is_err() {
                    if let Err(Error::InfeasibleWithCertificate(cert)) = &res_set_bound {
                        // println!("Cert: {:?}", cert);

                        let is_certif_valid_float = self.solver.problem.is_certificate_valid(cert);
                        let is_certif_valid_int = self.solver.check_certificate(cert, &mut self.stats).is_some();
                        return Some((is_certif_valid_int, is_certif_valid_float));
                    }

                    if let Err(Error::InfeasibleWithCertificate(cert)) = &res_check_feas {
                        // println!("Cert: {:?}", cert);

                        let is_certif_valid_float = self.solver.problem.is_certificate_valid(cert);
                        let is_certif_valid_int = self.solver.check_certificate(cert, &mut self.stats).is_some();
                        return Some((is_certif_valid_int, is_certif_valid_float));
                    }

                    break;
                }
            }

            None
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
        let mut lp_reasonner = Lp::new();

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

    fn compile_stats_certificate(nb_var: usize, nb_const: usize, min: IntCst, max: IntCst) {
        let n = 1000;

        let mut nb_val_cert_i = 0;

        let mut nb_val_cert_f = 0;

        let mut nb_cert = 0;

        for seed in 0..n {
            let (mut lp, _) = gen_filled_lp_domain(nb_var, nb_const, min, max, 0.1, seed);

            if let Some((is_val_cert_i, is_val_cert_f)) = lp.get_validity_certificates() {
                nb_val_cert_i += is_val_cert_i as usize;
                nb_val_cert_f += is_val_cert_f as usize;

                nb_cert += 1;
            }
        }

        println!("float: {nb_val_cert_f}, int: {nb_val_cert_i}, total: {nb_cert}");
    }

    #[ignore]
    #[test]
    fn compile_stats_certificate_single() {
        compile_stats_certificate(50, 100, -10, 10);
    }

    #[ignore]
    #[test]
    fn compile_stats_certificate_multiple() {
        for max in [10, 1000, INT_CST_MAX] {
            for nb_var in [30, 50, 100, 150] {
                for nb_const in [50, 100, 200, 400] {
                    print!("nb_var:{nb_var}, nb_const: {nb_const}, max: {max}, ");
                    compile_stats_certificate(
                        nb_var,
                        nb_const,
                        if max == INT_CST_MAX { INT_CST_MIN } else { -max },
                        max,
                    );
                }
            }
        }
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
