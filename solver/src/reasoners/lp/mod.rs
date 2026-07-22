mod solver;

use std::collections::HashMap;

use aries_env_param::EnvParam;
use solver::Solver;

use minilp::{Bound, Error, Variable};

use crate::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail},
    core::{
        IntCst, Lit, Var,
        literals::Watches,
        state::{Domains, DomainsSnapshot, Event, Explanation, InferenceCause},
    },
    lang::linear::{LinSum, ScaledVar},
    reasoners::{Contradiction, ReasonerId, Theory},
};

pub static LP_ACTIVE: EnvParam<bool> = EnvParam::new("ARIES_LP_ACTIVE", "true");

#[derive(Debug, Clone, Copy)]
struct BoundConstraint {
    var: Variable,
    bound: Bound,
    val: IntCst,
}

#[derive(Clone)]
struct LpEvent {
    var: Variable,
    bound: Bound,
    old_val: IntCst,
    old_lit: Lit,
}

#[derive(Clone)]
struct Stats {
    nb_certif: usize,
    nb_valid_certif: usize,
}

impl Stats {
    fn new() -> Self {
        Self {
            nb_certif: 0,
            nb_valid_certif: 0,
        }
    }

    fn increment_certif(&mut self) {
        self.nb_certif += 1;
    }

    fn increment_valid_certif(&mut self) {
        self.nb_valid_certif += 1;
    }
}

#[derive(Clone)]
pub struct Lp {
    id: ReasonerId,

    solver: Solver,

    bound_cons_vec: Vec<BoundConstraint>,
    lit_vec: Vec<Lit>,

    memory_s: HashMap<Vec<ScaledVar>, Variable>,
    memory_x: HashMap<Var, Variable>,

    model_events: ObsTrailCursor<Event>,
    watches: Watches<usize>,
    trail: Trail<LpEvent>,

    stats: Stats,

    active: bool,
}

impl Default for Lp {
    fn default() -> Self {
        Self::new()
    }
}

impl Lp {
    /// Create a new empty interface instance
    pub fn new() -> Self {
        Self {
            id: ReasonerId::Cp,
            solver: Solver::new(),

            bound_cons_vec: Vec::new(),
            lit_vec: Vec::new(),

            memory_s: HashMap::new(),
            memory_x: HashMap::new(),

            model_events: ObsTrailCursor::new(),
            watches: Default::default(),
            trail: Default::default(),

            stats: Stats::new(),

            active: LP_ACTIVE.get(),
        }
    }

    // Return a linear sum wich is the opposite in termes of coefficient that the one given
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

    fn add_x_var(&mut self, x: Var, doms: &Domains) {
        let var = self.solver.create_variable(doms.lb(x), doms.ub(x));

        self.memory_x.insert(x, var);
    }

    fn add_s_var(&mut self, linear_sum: &[ScaledVar], doms: &Domains) -> Variable {
        for &svar in linear_sum {
            if !self.memory_x.contains_key(&svar.var) {
                self.add_x_var(svar.var, doms);
            }
        }

        // If we depend on only one x variable with a factor 1, no need to create a s var
        if linear_sum.len() == 1 && linear_sum[0].factor == 1 {
            return *self.memory_x.get(&linear_sum[0].var).unwrap();
        }

        let s = self.solver.create_variable(IntCst::MIN, IntCst::MAX);

        let mut constraint = vec![(s, -1)];

        for &svar in linear_sum {
            let var = *self.memory_x.get(&svar.var).unwrap();
            constraint.push((var, svar.factor));
        }

        // We force s to be egal to our linear sum
        self.solver.add_constraint(constraint);

        self.memory_s.insert(linear_sum.to_vec(), s);

        s
    }

    pub fn process_constraint(&mut self, sum: &LinSum, active: Lit, doms: &Domains) {
        assert!(doms.presence(active) == Lit::TRUE);

        let bound_val = -sum.constant();

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

        let index = self.bound_cons_vec.len();

        self.watches.add_watch(index, active);
        self.bound_cons_vec.push(bound_cons);
        self.lit_vec.push(active);
    }

    fn explain_set_bound(&self, res: Result<(), Error>, var: Variable) -> Result<(), Contradiction> {
        match res {
            Err(Error::InfeasibleWithCertificate(cert)) => match self.solver.check_certificate(&cert) {
                Some(explanation) => {
                    // println!("{:?}", explanation);
                    Err(Contradiction::Explanation(explanation))
                }
                None => Ok(()),
            },

            Err(Error::Infeasible) => {
                let explanation = self.solver.explain_infeasible_var(var);
                Err(Contradiction::Explanation(explanation))
            }
            _ => Ok(()),
        }
    }

    fn explain_check_feas(&self, res: Result<(), Error>) -> Result<(), Contradiction> {
        match res {
            Err(Error::InfeasibleWithCertificate(cert)) => match self.solver.check_certificate(&cert) {
                Some(explanation) => Err(Contradiction::Explanation(explanation)),
                None => Ok(()),
            },
            _ => Ok(()),
        }
    }
}

impl Theory for Lp {
    fn identity(&self) -> ReasonerId {
        self.id
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        if !self.active {
            return Ok(());
        }

        while let Some(&event) = self.model_events.pop(domains.trail()) {
            let lit = event.new_literal();

            for watcher in self.watches.watches_on(lit) {
                let bound_cons = &self.bound_cons_vec[watcher];
                let active_lit = self.lit_vec[watcher];

                let res = self.solver.set_bound_restrict(
                    bound_cons.var,
                    bound_cons.bound,
                    bound_cons.val,
                    active_lit,
                    &mut self.trail,
                );

                self.explain_set_bound(res, bound_cons.var)?;
            }

            let var = event.affected_bound.variable();

            if let Some(&x_var) = self.memory_x.get(&var) {
                let res =
                    // if we have is plus, the constraint is of the form x <= b therefore it's an upper bound
                    if event.affected_bound.is_plus() {
                        self.solver
                            .set_bound_restrict(x_var, Bound::Upper, event.new_upper_bound, lit, &mut self.trail)
                    } else {
                        self.solver
                            .set_bound_restrict(x_var, Bound::Lower, -event.new_upper_bound, lit, &mut self.trail)
                    };

                self.explain_set_bound(res, x_var)?;
            }
        }

        let res = self.solver.check_feasability();

        self.explain_check_feas(res)?;

        Ok(())
    }

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
        if self.active {
            println!("ENABLED");
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
            let res = self
                .solver
                .set_bound(lp_event.var, lp_event.bound, lp_event.old_val, lp_event.old_lit);

            // If we have an error, that means our solver is in an instable state, therefore we reset it
            // if res.is_err() {
            //     println!("toto");
            //     self.solver.reset();
            // }
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

    use crate::{core::state::Cause, reasoners::cp::testing::pick_decisions};

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
            let bound_cons_vec = self.bound_cons_vec.clone();

            for bound_cons in bound_cons_vec {
                let res_set_bound = self
                    .solver
                    .set_bound(bound_cons.var, bound_cons.bound, bound_cons.val, Lit::TRUE);

                let res_check_feas = self.solver.check_feasability();

                if res_set_bound.is_err() || res_check_feas.is_err() {
                    if let Err(Error::InfeasibleWithCertificate(cert)) = &res_set_bound {
                        // println!("Cert: {:?}", cert);

                        let is_certif_valid_float = self.solver.problem.is_certificate_valid(cert);
                        let is_certif_valid_int = self.solver.check_certificate(cert).is_some();
                        return Some((is_certif_valid_int, is_certif_valid_float));
                    }

                    if let Err(Error::InfeasibleWithCertificate(cert)) = &res_check_feas {
                        // println!("Cert: {:?}", cert);

                        let is_certif_valid_float = self.solver.problem.is_certificate_valid(cert);
                        let is_certif_valid_int = self.solver.check_certificate(cert).is_some();
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

            lp_reasonner.process_constraint(&sum, active, &d);
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
        for max in [10, 1000, i32::MAX] {
            for nb_var in [30, 50, 100, 150] {
                for nb_const in [50, 100, 200, 400] {
                    print!("nb_var:{nb_var}, nb_const: {nb_const}, max: {max}, ");
                    compile_stats_certificate(nb_var, nb_const, if max == i32::MAX { i32::MIN } else { -max }, max);
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

        lp.save_state();

        let mut nb_explanation = 0;

        // repeat a large number of random tests
        for _ in 0..100 {
            let mut lp = lp.clone();

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
                        Contradiction::InvalidUpdate(_) => Explanation::new(), // Unreachable branch as our lp never returns InvalidUpdate
                    };
                    lp.reset();

                    // println!("\nBounds 2:\n {:?}", lp.solver.bounds);
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
                        lp.propagate(&mut d).is_err(),
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
}
