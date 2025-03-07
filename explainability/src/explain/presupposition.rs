use std::sync::Arc;

use aries::backtrack::{Backtrack, DecLvl};
use aries::model::lang::expr::and;
use aries::model::{Label, Model};
use aries::solver::Solver;
use itertools::Itertools;

use crate::explain::{Query, Situation};

/// Represents a fact that a question assumes to be true,
/// and thus must be true for the question to make sense.
pub struct Presupposition<Lbl: Label> {
    pub kind: PresuppositionKind,
    pub model: Arc<Model<Lbl>>,
    pub situ: Situation,
    pub query: Query,
}

/// Our possible kinds of presuppositions.
/// All of them implicitly require the model to be satisfiable with the situation.
#[derive(Debug, Clone, Copy)]
pub enum PresuppositionKind {
    /// The model must be unsatisfiable with the situation and query. (But satisfiable with just the situation).
    ModelSituUnsatWithQuery,
    /// The model must be satisfiable with the situation and query.
    ModelSituSatWithQuery,
    /// The model and situation being satisfied together must entail (necessarily satisfy) the query.
    ModelSituEntailQuery,
    /// The model and situation being satisfied together must not entail (not necessarily (or at all) satisfy) the query.
    ModelSituNotEntailQuery,
}

/// The possible reasons behind a presupposition holding or not.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PresuppositionStatusCause {
    ModelSituUnsat,
    ModelSituQueryUnsat,
    ModelSituQuerySat,
    ModelSituNegQueryUnsat,
    ModelSituNegQuerySat,
}

impl<Lbl: Label> Presupposition<Lbl> {
    /// Checks if the presupposition holds.
    /// Returns its "status", i.e. the reason behind it holding (`Ok`) or not holding (`Err`).
    pub fn check(
        &self,
        skip_model_situ_sat_check: bool,
        init_base_solver_fn: fn(Model<Lbl>) -> Solver<Lbl>,
        solve_fn: impl Fn(&mut Solver<Lbl>) -> bool, //Result<(), ()>, //aries::solver::parallel::SolverResult<(Arc<aries::core::state::Domains>, Option<aries::core::IntCst>)>
    ) -> Result<PresuppositionStatusCause, PresuppositionStatusCause> {
        let mut base_solver = init_base_solver_fn((*self.model).clone());

        assert_eq!(base_solver.current_decision_level(), DecLvl::ROOT);
        let query_neg = !base_solver.reify(and(self.query.iter().copied().collect_vec()));

        let res = {
            if base_solver.propagate_and_backtrack_to_consistent().is_err() {
                return Err(PresuppositionStatusCause::ModelSituUnsat);
            }
            if base_solver.incremental_push_all(self.situ.iter().copied()).is_err() {
                return Err(PresuppositionStatusCause::ModelSituUnsat);
            }
            if skip_model_situ_sat_check {
                // If we (the caller of this function) want to skip checking `model` /\ `situ` being SAT (because
                // we already know that it's the case), then we only needed to do assumptions and propagations (but not the solving).
            // } else if _base_solver.incremental_solve().expect("Solver interrupted").is_err() {
            } else if !solve_fn(&mut base_solver) {
                return Err(PresuppositionStatusCause::ModelSituUnsat);
            }
            debug_assert_eq!(base_solver.model.state.assumptions().into_iter().collect::<Situation>(), self.situ);

            base_solver.reset_search();

            let query = self.query.iter().copied();

            match self.kind {
                PresuppositionKind::ModelSituUnsatWithQuery => {
                    if base_solver.incremental_push_all(query).is_err() {
                        return Ok(PresuppositionStatusCause::ModelSituQueryUnsat);
                    }
                    match solve_fn(&mut base_solver) {
                        true => {
                            Err(PresuppositionStatusCause::ModelSituQuerySat)
                        },
                        false => Ok(PresuppositionStatusCause::ModelSituQueryUnsat),
                    }
                }
                PresuppositionKind::ModelSituSatWithQuery => {
                    if base_solver.incremental_push_all(query).is_err() {
                        return Err(PresuppositionStatusCause::ModelSituQueryUnsat);
                    }
                    match solve_fn(&mut base_solver) {
                        true => Ok(PresuppositionStatusCause::ModelSituQuerySat),
                        false => Err(PresuppositionStatusCause::ModelSituQueryUnsat),
                    }
                }
                PresuppositionKind::ModelSituEntailQuery => {
                    if base_solver.incremental_push_all(query).is_err() {
                        return Err(PresuppositionStatusCause::ModelSituQueryUnsat);
                    }
                    for _ in 0..self.query.len() {
                        base_solver.incremental_pop();
                    }
                    if base_solver.incremental_push(query_neg).is_err(){
                        return Ok(PresuppositionStatusCause::ModelSituNegQueryUnsat);
                    }
                    match solve_fn(&mut base_solver) {
                        true => Err(PresuppositionStatusCause::ModelSituNegQuerySat),
                        false => Ok(PresuppositionStatusCause::ModelSituNegQueryUnsat),
                    }
                }
                PresuppositionKind::ModelSituNotEntailQuery => {
                    if base_solver.incremental_push_all(query).is_err() {
                        return Ok(PresuppositionStatusCause::ModelSituQueryUnsat);
                    }
                    for _ in 0..self.query.len() {
                        base_solver.incremental_pop();
                    }
                    if base_solver.incremental_push(query_neg).is_err(){
                        return Err(PresuppositionStatusCause::ModelSituNegQueryUnsat);
                    }
                    match solve_fn(&mut base_solver) {
                        true => Ok(PresuppositionStatusCause::ModelSituNegQuerySat),
                        false => Err(PresuppositionStatusCause::ModelSituNegQueryUnsat),
                    }
                }
            }
        };
        // necessary if the solver was a cached one (given as parameter),
        // to ensure it can be safely reused somewhere else.
        base_solver.reset();
        
        let res_is_valid = match self.kind {
            PresuppositionKind::ModelSituUnsatWithQuery => {
                matches!(
                    res,
                    Err(PresuppositionStatusCause::ModelSituUnsat)
                    | Ok(PresuppositionStatusCause::ModelSituQueryUnsat)
                    | Err(PresuppositionStatusCause::ModelSituQuerySat),
                )
            }
            PresuppositionKind::ModelSituSatWithQuery => {
                matches!(
                    res,
                    Err(PresuppositionStatusCause::ModelSituUnsat)
                    | Err(PresuppositionStatusCause::ModelSituQueryUnsat)
                    | Ok(PresuppositionStatusCause::ModelSituQuerySat),
                )
            }
            PresuppositionKind::ModelSituEntailQuery => {
                matches!(
                    res,
                    Err(PresuppositionStatusCause::ModelSituUnsat)
                    | Err(PresuppositionStatusCause::ModelSituQueryUnsat)
                    | Err(PresuppositionStatusCause::ModelSituNegQuerySat)
                    | Ok(PresuppositionStatusCause::ModelSituNegQueryUnsat),
                )
            }
            PresuppositionKind::ModelSituNotEntailQuery => {
                matches!(
                    res,
                    Err(PresuppositionStatusCause::ModelSituUnsat)
                    | Ok(PresuppositionStatusCause::ModelSituQueryUnsat)
                    | Ok(PresuppositionStatusCause::ModelSituNegQuerySat)
                    | Err(PresuppositionStatusCause::ModelSituNegQueryUnsat),
                )
            }
        };
        debug_assert!(res_is_valid);

        res
    }
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use aries::model::lang::expr::lt;

    use crate::explain::presupposition::{Presupposition, PresuppositionKind, PresuppositionStatusCause};
    use crate::explain::{Query, Situation};

    type Lbl = &'static str;
    type Model = aries::model::Model<Lbl>;

    #[test]
    fn test_presupposition_model_situ_unsat() {
        let mut model = Model::new();

        let x = model.new_ivar(0, 5, "x");
        let y = model.new_ivar(0, 5, "y");
        let z = model.new_ivar(0, 5, "z");

        let xlty = model.reify(lt(x, y));
        let yltz = model.reify(lt(y, z));
        let zltx = model.reify(lt(z, x));

        let model = Arc::new(model);
        let init_base_solver_fn = |m| {
            let stn_config = aries::reasoners::stn::theory::StnConfig {
                theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
                ..Default::default()
            };
            let mut solver = aries::solver::Solver::<Lbl>::new(m);
            solver.reasoners.diff.config = stn_config;
            solver
        };
        let solve_fn = |s: &mut aries::solver::Solver<Lbl>| s.solve().is_ok_and(|a| a.is_some());

        let test_fn = |kind: PresuppositionKind| {
            let presupp = Presupposition {
                kind,
                model: model.clone(),
                situ: Situation::from([xlty, yltz, zltx]),
                query: Query::from([]),
            };
            assert_eq!(
                presupp.check(false, init_base_solver_fn, solve_fn),
                Err(PresuppositionStatusCause::ModelSituUnsat)
            );
        };

        test_fn(PresuppositionKind::ModelSituUnsatWithQuery);
        test_fn(PresuppositionKind::ModelSituSatWithQuery);
        test_fn(PresuppositionKind::ModelSituEntailQuery);
        test_fn(PresuppositionKind::ModelSituNotEntailQuery);
    }

    #[test]
    fn test_presupposition_model_situ_unsat_with_query() {
        let kind = PresuppositionKind::ModelSituUnsatWithQuery;

        let mut model = Model::new();

        let x = model.new_ivar(0, 5, "x");
        let y = model.new_ivar(0, 5, "y");
        let z = model.new_ivar(0, 5, "z");

        let xlty = model.reify(lt(x, y));
        let yltz = model.reify(lt(y, z));
        let zltx = model.reify(lt(z, x));

        let model = Arc::new(model);
        let init_base_solver_fn = |m| {
            let stn_config = aries::reasoners::stn::theory::StnConfig {
                theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
                ..Default::default()
            };
            let mut solver = aries::solver::Solver::<Lbl>::new(m);
            solver.reasoners.diff.config = stn_config;
            solver
        };
        let solve_fn = |s: &mut aries::solver::Solver<Lbl>| s.solve().is_ok_and(|a| a.is_some());

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([]),
            query: Query::from([xlty, yltz, zltx]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Ok(PresuppositionStatusCause::ModelSituQueryUnsat)
        );

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([]),
            query: Query::from([xlty, yltz]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Err(PresuppositionStatusCause::ModelSituQuerySat)
        );

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([z.geq(1), z.leq(1)]),
            query: Query::from([xlty, yltz]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Ok(PresuppositionStatusCause::ModelSituQueryUnsat)
        );  
    }

    #[test]
    fn test_presupposition_model_situ_sat_with_query() {
        let kind = PresuppositionKind::ModelSituSatWithQuery;

        let mut model = Model::new();

        let x = model.new_ivar(0, 5, "x");
        let y = model.new_ivar(0, 5, "y");
        let z = model.new_ivar(0, 5, "z");

        let xlty = model.reify(lt(x, y));
        let yltz = model.reify(lt(y, z));
        let zltx = model.reify(lt(z, x));

        let model = Arc::new(model);
        let init_base_solver_fn = |m| {
            let stn_config = aries::reasoners::stn::theory::StnConfig {
                theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
                ..Default::default()
            };
            let mut solver = aries::solver::Solver::<Lbl>::new(m);
            solver.reasoners.diff.config = stn_config;
            solver
        };
        let solve_fn = |s: &mut aries::solver::Solver<Lbl>| s.solve().is_ok_and(|a| a.is_some());

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([]),
            query: Query::from([xlty, yltz, zltx]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Err(PresuppositionStatusCause::ModelSituQueryUnsat)
        );

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([]),
            query: Query::from([xlty, yltz]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Ok(PresuppositionStatusCause::ModelSituQuerySat)
        );

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([z.leq(2)]),
            query: Query::from([xlty, yltz]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Ok(PresuppositionStatusCause::ModelSituQuerySat)
        );  
    }

    #[test]
    fn test_presupposition_model_situ_entail_query() {
        let kind = PresuppositionKind::ModelSituEntailQuery;

        let mut model = Model::new();

        let x = model.new_ivar(0, 5, "x");
        let y = model.new_ivar(0, 5, "y");
        let z = model.new_ivar(0, 5, "z");

        let xlty = model.reify(lt(x, y));
        let yltz = model.reify(lt(y, z));
        let zltx = model.reify(lt(z, x));

        let model = Arc::new(model);
        let init_base_solver_fn = |m| {
            let stn_config = aries::reasoners::stn::theory::StnConfig {
                theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
                ..Default::default()
            };
            let mut solver = aries::solver::Solver::<Lbl>::new(m);
            solver.reasoners.diff.config = stn_config;
            solver
        };
        let solve_fn = |s: &mut aries::solver::Solver<Lbl>| s.solve().is_ok_and(|a| a.is_some());

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([]),
            query: Query::from([xlty, yltz, zltx]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Err(PresuppositionStatusCause::ModelSituQueryUnsat)
        );

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([]),
            query: Query::from([xlty, yltz]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Err(PresuppositionStatusCause::ModelSituNegQuerySat)
        );

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([xlty]),
            query: Query::from([y.geq(1)]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Ok(PresuppositionStatusCause::ModelSituNegQueryUnsat)
        );
    }

    #[test]
    fn test_presupposition_model_situ_not_entail_query() {
        let kind = PresuppositionKind::ModelSituNotEntailQuery;

        let mut model = Model::new();

        let x = model.new_ivar(0, 5, "x");
        let y = model.new_ivar(0, 5, "y");
        let z = model.new_ivar(0, 5, "z");

        let xlty = model.reify(lt(x, y));
        let yltz = model.reify(lt(y, z));
        let zltx = model.reify(lt(z, x));

        let model = Arc::new(model);
        let init_base_solver_fn = |m| {
            let stn_config = aries::reasoners::stn::theory::StnConfig {
                theory_propagation: aries::reasoners::stn::theory::TheoryPropagationLevel::Full,
                ..Default::default()
            };
            let mut solver = aries::solver::Solver::<Lbl>::new(m);
            solver.reasoners.diff.config = stn_config;
            solver
        };
        let solve_fn = |s: &mut aries::solver::Solver<Lbl>| s.solve().is_ok_and(|a| a.is_some());

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([]),
            query: Query::from([xlty, yltz, zltx]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Ok(PresuppositionStatusCause::ModelSituQueryUnsat)
        );

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([]),
            query: Query::from([xlty, yltz]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Ok(PresuppositionStatusCause::ModelSituNegQuerySat)
        );

        let presupp = Presupposition {
            kind,
            model: model.clone(),
            situ: Situation::from([xlty]),
            query: Query::from([y.geq(1)]),
        };
        assert_eq!(
            presupp.check(false, init_base_solver_fn, solve_fn),
            Err(PresuppositionStatusCause::ModelSituNegQueryUnsat)
        );
    }
}
