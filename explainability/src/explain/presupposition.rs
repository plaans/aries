use std::sync::Arc;

use aries::backtrack::{Backtrack, DecLvl};
use aries::model::{Label, Model};
use aries::reasoners::stn::theory::{StnConfig, TheoryPropagationLevel};
use aries::solver::Solver;
use itertools::Itertools;

use crate::explain::{Query, Situation};

pub struct Presupposition<Lbl: Label> {
    pub kind: PresuppositionKind,
    pub model: Arc<Model<Lbl>>,
    pub situ: Situation,
    pub query: Query,
}
impl<Lbl: Label> std::fmt::Debug for Presupposition<Lbl> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("Presupposition")
            .field("kind", &self.kind)
            .field("model", &"model")
            .field("situ", &self.situ)
            .field("query", &self.query)
            .finish()
    }
}

#[derive(Debug)]
pub struct UnmetPresupposition<Lbl: Label> {
    presupposition: Presupposition<Lbl>,
    cause: UnmetPresuppositionCause,
}

#[derive(Debug)]
pub enum PresuppositionKind {
    ModelSituUnsatWithQuery,
    ModelSituSatWithQuery,
    ModelSituNotEntailQuery,
    ModelSituEntailQuery,
}

#[derive(Debug)]
pub enum UnmetPresuppositionCause {
    ModelSituUnsat,
    ModelSituQueryUnsat,
    ModelSituQuerySat,
    ModelSituNegQueryUnsat,
    ModelSituNegQuerySat,
}

pub fn check_presupposition<Lbl: Label>(
    presupposition: Presupposition<Lbl>,
    skip_model_situ_sat_check: bool,
    cached_solver: Option<&mut Solver<Lbl>>,
) -> Result<(), UnmetPresupposition<Lbl>> {
    let solver = if let Some(s) = cached_solver {
        s
    } else {
        &mut create_solver((*presupposition.model).clone())
    };
    if skip_model_situ_sat_check {
        debug_assert!(solver.current_decision_level() == DecLvl::ROOT);
        match solver.propagate_and_backtrack_to_consistent(solver.current_decision_level()) {
            Ok(_) => (), // expected,
            Err(_) => debug_assert!(false),
        }
        for &lit in &presupposition.situ {
            match solver.assume(lit) {
                Ok(_) => (), // expected
                Err(_) => debug_assert!(false),
            }
        }
    } else {
        match solver.solve_with_assumptions(presupposition.situ.clone()) {
            Ok(_) => solver.restore(DecLvl::from(presupposition.situ.len())),
            Err(_) => {
                return Err(UnmetPresupposition {
                    presupposition,
                    cause: UnmetPresuppositionCause::ModelSituUnsat,
                })
            }
        }
    }

    // Remember, `situ` is already assumed (we backtracked to the latest assumption).
    debug_assert!(solver.current_decision_level() == DecLvl::from(presupposition.situ.len()));
    // And so, we will just use `query` (or `query_neg`) in `solve_with_assumptions` calls below (incremental solving).

    let res = match presupposition.kind {
        PresuppositionKind::ModelSituUnsatWithQuery => {
            match solver
                .solve_with_assumptions(presupposition.query.clone())
                .expect("Solver interrupted.")
            {
                Ok(_) => Err(UnmetPresupposition {
                    presupposition,
                    cause: UnmetPresuppositionCause::ModelSituQuerySat,
                }),
                Err(_) => Ok(()),
            }
        }
        PresuppositionKind::ModelSituSatWithQuery => {
            match solver
                .solve_with_assumptions(presupposition.query.clone())
                .expect("Solver interrupted.")
            {
                Ok(_) => Ok(()),
                Err(_) => Err(UnmetPresupposition {
                    presupposition,
                    cause: UnmetPresuppositionCause::ModelSituQueryUnsat,
                }),
            }
        }
        PresuppositionKind::ModelSituNotEntailQuery => {
            let dl = DecLvl::from(presupposition.query.len());
            match solver
                .solve_with_assumptions(presupposition.query.clone())
                .expect("Solver interrupted.")
            {
                Ok(_) => {
                    solver.restore(dl);
                    let query_neg = presupposition.query.iter().map(|&l| !l).collect_vec();
                    match solver.solve_with_assumptions(query_neg).expect("Solver interrupted.") {
                        Ok(_) => Err(UnmetPresupposition {
                            presupposition,
                            cause: UnmetPresuppositionCause::ModelSituNegQuerySat,
                        }),
                        Err(_) => Ok(()),
                    }
                }
                Err(_) => Err(UnmetPresupposition {
                    presupposition,
                    cause: UnmetPresuppositionCause::ModelSituQueryUnsat,
                }),
            }
        }
        PresuppositionKind::ModelSituEntailQuery => {
            let neg_query = presupposition.query.iter().map(|&l| !l).collect_vec();
            match solver.solve_with_assumptions(neg_query).expect("Solver interrupted.") {
                Ok(_) => Ok(()),
                Err(_) => Err(UnmetPresupposition {
                    presupposition,
                    cause: UnmetPresuppositionCause::ModelSituNegQueryUnsat,
                }),
            }
        }
    };
    // necessary if the solver was a cached one (given as parameter), to ensure it can be safely reused somewhere else.
    solver.reset();
    res
}

fn create_solver<Lbl: Label>(model: Model<Lbl>) -> Solver<Lbl> {
    let stn_config = StnConfig {
        theory_propagation: TheoryPropagationLevel::Full,
        ..Default::default()
    };
    let mut solver = Solver::<Lbl>::new(model);
    solver.reasoners.diff.config = stn_config;
    solver
}