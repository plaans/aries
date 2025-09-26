//! This module exports an alternate propagator for equality logic.
//!
//! Since DenseEqTheory has O(n^2) space complexity it tends to have performance issues on larger problems.
//! This alternative has much lower memory use on sparse problems, and can make stronger inferences than just the STN
//!
//! Currently, this propagator is intended to be used in conjunction with the StnTheory.
//! Each l => x = y constraint should be posted as l => x >= y and l => x <= y,
//! and each l => x != y constraint should be posted as l => x > y or l => x < y in the STN.
//! This is because AltEqTheory does not do bound propagation yet
//! (When a integer variable's bounds are updated, no propagation occurs).
//! Stn is therefore ideally used in "bounds" propagation mode ("edges" is redundant) with this propagator.

// TODO: Implement bound propagation for this theory.

mod constraints;
mod graph;
mod node;
mod relation;
mod theory;

pub use theory::AltEqTheory;

#[cfg(test)]
mod tests {
    use std::fmt::Display;

    use itertools::Itertools;
    use rand::{rngs::SmallRng, seq::IteratorRandom, Rng, SeedableRng};

    use crate::{
        core::{
            state::{Cause, Domains},
            IntCst, Lit, VarRef,
        },
        model::{
            lang::{
                expr::{and, eq, geq, gt, leq, lt, neq, or},
                IVar,
            },
            Model,
        },
        solver::{search::random::RandomChoice, Solver},
    };

    use super::relation::EqRelation;

    struct Problem {
        domains: Domains,
        constraints: Vec<(VarRef, VarRef, EqRelation, bool, bool)>,
    }

    const VARS_PER_PROBLEM: usize = 20;

    fn generate_problem(rng: &mut SmallRng) -> Problem {
        // Calibrated for approximately equal number of solvable and unsolvable problems
        let sparsity = 0.5;
        let neq_probability = 0.5;
        let full_reif_probability = 0.5;
        let enforce_probability = 0.5;
        let max_scopes = 5;

        let mut domains = Domains::new();

        let num_scopes = rng.gen_range(1..max_scopes);

        let mut scopes = vec![Lit::TRUE];
        for i in 1..num_scopes {
            scopes.push(domains.new_presence_literal(scopes[i - 1]));
        }

        // Lit::TRUE, Lit::FALSE, and scopes other than TRUE
        let var_offset = num_scopes - 1 + 2;

        for i in var_offset..VARS_PER_PROBLEM + var_offset {
            assert_eq!(
                VarRef::from(i),
                domains.new_optional_var(0, VARS_PER_PROBLEM as IntCst - 1, *scopes.iter().choose(rng).unwrap())
            );
        }

        #[allow(clippy::filter_map_bool_then)] // Avoids double borrowing rng
        let constraints = (var_offset..VARS_PER_PROBLEM + var_offset)
            .tuple_combinations()
            .filter_map(|(a, b)| {
                rng.gen_bool(sparsity).then(|| {
                    (
                        a.into(),
                        b.into(),
                        if rng.gen_bool(neq_probability) {
                            EqRelation::Neq
                        } else {
                            EqRelation::Eq
                        },
                        rng.gen_bool(full_reif_probability),
                        rng.gen_bool(enforce_probability),
                    )
                })
            })
            .collect_vec();
        Problem { domains, constraints }
    }

    #[derive(Debug, Hash, PartialEq, Eq, Clone)]
    enum Label {
        ReifLiteral(VarRef, VarRef),
    }

    impl Display for Label {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "{:?}", self)
        }
    }

    fn model_with_eq(problem: &Problem) -> Model<Label> {
        let mut model = Model::new().with_domains(problem.domains.clone());
        for (a, b, r, full_reif, enforce) in problem.constraints.clone() {
            let l = if full_reif {
                match r {
                    EqRelation::Eq => model.reify(eq(IVar::new(a), IVar::new(b))),
                    EqRelation::Neq => model.reify(neq(IVar::new(a), IVar::new(b))),
                }
            } else {
                match r {
                    EqRelation::Eq => model.half_reify(eq(IVar::new(a), IVar::new(b))),
                    EqRelation::Neq => model.half_reify(neq(IVar::new(a), IVar::new(b))),
                }
            };
            if enforce {
                model.state.set(l, Cause::Encoding).unwrap();
            }
            model.shape.labels.insert(l.variable(), Label::ReifLiteral(a, b));
        }
        model
    }

    fn model_with_stn(problem: &Problem) -> Model<Label> {
        let mut model = Model::new().with_domains(problem.domains.clone());
        for (a, b, r, full_reif, enforce) in problem.constraints.clone() {
            let l = if full_reif {
                match r {
                    EqRelation::Eq => {
                        let l1 = model.reify(leq(IVar::new(a), IVar::new(b)));
                        let l2 = model.reify(geq(IVar::new(a), IVar::new(b)));
                        model.reify(and(vec![l1, l2].into_boxed_slice()))
                    }
                    EqRelation::Neq => {
                        let l1 = model.reify(lt(IVar::new(a), IVar::new(b)));
                        let l2 = model.reify(gt(IVar::new(a), IVar::new(b)));
                        model.reify(or(vec![l1, l2].into_boxed_slice()))
                    }
                }
            } else {
                match r {
                    EqRelation::Eq => {
                        let l1 = model.half_reify(leq(IVar::new(a), IVar::new(b)));
                        let l2 = model.half_reify(geq(IVar::new(a), IVar::new(b)));
                        model.reify(and(vec![l1, l2].into_boxed_slice()))
                    }
                    EqRelation::Neq => {
                        let l1 = model.half_reify(lt(IVar::new(a), IVar::new(b)));
                        let l2 = model.half_reify(gt(IVar::new(a), IVar::new(b)));
                        model.reify(or(vec![l1, l2].into_boxed_slice()))
                    }
                }
            };
            if enforce {
                model.state.set(l, Cause::Encoding).unwrap();
            }
            model.shape.labels.insert(l.variable(), Label::ReifLiteral(a, b));
        }
        model
    }

    #[test]
    fn test_random_order() {
        let mut rng = SmallRng::seed_from_u64(0);
        let problems = (0..10).map(|_| generate_problem(&mut rng));
        for problem in problems {
            let model = model_with_eq(&problem);
            let mut solver = Solver::new(model.clone());
            solver.set_brancher(RandomChoice::new(0));
            let solution = solver.solve().unwrap();
            for i in 1..5 {
                let mut solver = Solver::new(model.clone());
                solver.set_brancher(RandomChoice::new(i));
                let new_solution = solver.solve().unwrap();

                assert_eq!(new_solution.is_some(), solution.is_some());
            }
        }
    }

    #[test]
    fn test_vs_stn() {
        let mut rng = SmallRng::seed_from_u64(0);
        let problems = (0..10).map(|_| generate_problem(&mut rng));
        for problem in problems {
            let eq_model = model_with_eq(&problem);
            let mut eq_solver = Solver::new(eq_model.clone());
            eq_solver.set_brancher(RandomChoice::new(0));
            let eq_solution = eq_solver.solve().unwrap();
            let stn_model = model_with_stn(&problem);
            let mut stn_solver = Solver::new(stn_model.clone());
            stn_solver.set_brancher(RandomChoice::new(0));
            let stn_solution = stn_solver.solve().unwrap();

            assert_eq!(eq_solution.is_some(), stn_solution.is_some())
        }
    }
}
