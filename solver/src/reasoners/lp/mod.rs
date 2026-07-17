mod solver;

use std::collections::HashMap;

use solver::Solver;

use minilp::{Bound, Error, Variable};

use crate::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail},
    core::{
        IntCst, Lit, Var,
        literals::Watches,
        state::{Domains, DomainsSnapshot, Event, Explanation, InferenceCause, InvalidUpdate},
    },
    lang::linear::{LinSum, ScaledVar},
    reasoners::{Contradiction, ReasonerId, Theory},
};

#[derive(Debug, Clone)]
struct ActivationLit {
    var: Variable,
    bound: Bound,
    val: IntCst,
}

#[derive(Clone)]
enum LpEvent {
    BoundSet(ActivationLit, IntCst),
}

#[derive(Clone)]
pub struct Lp {
    id: ReasonerId,

    solver: Solver,

    act_lit_vec: Vec<ActivationLit>,

    memory_s: HashMap<Vec<ScaledVar>, Variable>,
    memory_x: HashMap<Var, Variable>,
    memory_x_reversed: HashMap<Variable, Var>,

    model_events: ObsTrailCursor<Event>,
    watches_act_lit: Watches<usize>,
    trail: Trail<LpEvent>,
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

            act_lit_vec: Vec::new(),

            memory_s: HashMap::new(),
            memory_x: HashMap::new(),
            memory_x_reversed: HashMap::new(),

            model_events: ObsTrailCursor::new(),
            watches_act_lit: Default::default(),
            trail: Default::default(),
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

    fn add_x_var(&mut self, x: Var) {
        let var = self.solver.create_variable(IntCst::MIN, IntCst::MAX);

        self.memory_x.insert(x, var);
        self.memory_x_reversed.insert(var, x);
    }

    fn add_s_var(&mut self, linear_sum: &[ScaledVar]) -> Variable {
        for &svar in linear_sum {
            if !self.memory_x.contains_key(&svar.var) {
                self.add_x_var(svar.var);
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

    pub fn process_constraint(&mut self, sum: &LinSum, active: Lit) {
        let bound_val = -sum.constant();

        let elements = sum.terms_slice().to_vec();

        let opp_lin_sum = Lp::get_opposite_linear_sum(&elements);

        let lit: ActivationLit;

        if self.memory_s.contains_key(&elements) {
            let &s = self.memory_s.get(&elements).unwrap();
            lit = ActivationLit {
                var: s,
                bound: Bound::Upper,
                val: bound_val,
            };
        } else if self.memory_s.contains_key(&opp_lin_sum) {
            let &s = self.memory_s.get(&opp_lin_sum).unwrap();
            lit = ActivationLit {
                var: s,
                bound: Bound::Lower,
                val: -bound_val,
            };
        } else {
            let s = self.add_s_var(&elements);

            lit = ActivationLit {
                var: s,
                bound: Bound::Upper,
                val: bound_val,
            };
        }

        let index = self.act_lit_vec.len();

        self.watches_act_lit.add_watch(index, active);
        self.act_lit_vec.push(lit);
    }

    fn explain_set_bound(&self, res: Result<(), Error>) -> Result<(), Contradiction> {
        match res {
            Err(Error::InfeasibleWithCertificate(cert)) => {
                if self.solver.is_certificate_valid(&cert) {
                    todo!()
                } else {
                    todo!()
                }
            }
            Err(Error::Instable) => todo!(),
            Err(_) => {
                todo!()
            }
            Ok(_) => {}
        }

        Ok(())
    }
}

impl Theory for Lp {
    fn identity(&self) -> ReasonerId {
        self.id
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        while let Some(&event) = self.model_events.pop(domains.trail()) {
            let lit = event.new_literal();

            for watcher in self.watches_act_lit.watches_on(lit) {
                let act_lit = &self.act_lit_vec[watcher];

                let res = self
                    .solver
                    .set_bound_restrict(act_lit.var, act_lit.bound, act_lit.val, &mut self.trail);

                self.explain_set_bound(res)?;
            }

            let var = event.affected_bound.variable();

            if let Some(&x_var) = self.memory_x.get(&var) {
                let res =
                    // if we have is plus, the constraint is of the form x <= b therefore it's an upper bound
                    if event.affected_bound.is_plus() {
                        self.solver
                            .set_bound_restrict(x_var, Bound::Upper, event.new_upper_bound, &mut self.trail)
                    } else {
                        self.solver
                            .set_bound_restrict(x_var, Bound::Lower, -event.new_upper_bound, &mut self.trail)
                    };

                self.explain_set_bound(res)?;
            }
        }

        Ok(())
    }

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        state: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        todo!()
    }

    fn print_stats(&self) {
        todo!()
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
        self.trail.restore_last_with(|LpEvent::BoundSet(lit, old_val)| {
            let _ = self.solver.set_bound(lit.var, lit.bound, old_val); // Should not return an error as we are only relaxing the lp
        });
    }
}

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng, rngs::SmallRng, seq::IteratorRandom};

    use crate::{
        core::{IntCst, Lit, Var},
        lang::linear::{LinSum, ScaledVar},
        reasoners::lp::{Error, Lp},
    };

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
            let lit_vec = self.act_lit_vec.clone();

            for lit in lit_vec {
                let res_set_bound = self.solver.set_bound(lit.var, lit.bound, lit.val);

                let res_check_feas = self.solver.check_feasability();

                if res_set_bound.is_err() || res_check_feas.is_err() {
                    if let Err(Error::InfeasibleWithCertificate(cert)) = &res_set_bound {
                        // println!("Cert: {:?}", cert);

                        let is_certif_valid_float = self.solver.problem.is_certificate_valid(cert);
                        let is_certif_valid_int = self.solver.is_certificate_valid(cert);
                        return Some((is_certif_valid_int, is_certif_valid_float));
                    }

                    if let Err(Error::InfeasibleWithCertificate(cert)) = &res_check_feas {
                        // println!("Cert: {:?}", cert);

                        let is_certif_valid_float = self.solver.problem.is_certificate_valid(cert);
                        let is_certif_valid_int = self.solver.is_certificate_valid(cert);
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

    fn gen_filled_lp(
        nb_var: usize,
        nb_const: usize,
        min: IntCst,
        max: IntCst,
        sparse_proportion: f32,
        seed: u64,
    ) -> Lp {
        let mut interface = Lp::new();

        let mut rng = SmallRng::seed_from_u64(seed);

        for _ in 0..nb_const {
            let nb_x = get_nb_x(sparse_proportion, &mut rng, nb_var);

            let x_vec: Vec<Var> = (0..nb_var)
                .choose_multiple(&mut rng, nb_x)
                .iter()
                .map(|&x| Var::from_u32((x + 1) as u32))
                .collect();

            let vars: Vec<ScaledVar> = x_vec
                .iter()
                .map(|&v| ScaledVar {
                    var: v,
                    factor: rng.random_range(min..=max),
                })
                .collect();

            // println!("constraint: {:?}", linear_sum);

            let bound_val = rng.random_range(min..=max);

            let sum = LinSum::new(bound_val, vars);

            interface.process_constraint(&sum, Lit::TRUE);
        }

        interface
    }

    fn compile_stats_certificate(nb_var: usize, nb_const: usize, min: IntCst, max: IntCst) {
        let n = 1000;

        let mut nb_val_cert_i = 0;

        let mut nb_val_cert_f = 0;

        let mut nb_cert = 0;

        for seed in 0..n {
            let mut interface = gen_filled_lp(nb_var, nb_const, min, max, 0.1, seed);

            if let Some((is_val_cert_i, is_val_cert_f)) = interface.get_validity_certificates() {
                nb_val_cert_i += is_val_cert_i as usize;
                nb_val_cert_f += is_val_cert_f as usize;

                nb_cert += 1;
            }
        }

        println!("float: {nb_val_cert_f}, int: {nb_val_cert_i}, total: {nb_cert}");
    }

    #[test]
    fn compile_stats_certificate_single() {
        compile_stats_certificate(50, 100, -10, 10);
    }

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
}
