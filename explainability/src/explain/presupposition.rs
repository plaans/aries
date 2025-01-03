use std::sync::Arc;

use aries::backtrack::{Backtrack, DecLvl};
use aries::model::{Label, Model};
use aries::reasoners::stn::theory::{StnConfig, TheoryPropagationLevel};
use aries::solver::Solver;

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
        // If we (the caller of the function) have supplied a cached solver to use, then use it.
        s
    } else {
        // If no cached solver has been supplied, then create one and use it.
        &mut {
            let model = (*presupposition.model).clone();
            let stn_config = StnConfig {
                theory_propagation: TheoryPropagationLevel::Full,
                ..Default::default()
            };
            let mut solver = Solver::<Lbl>::new(model);
            solver.reasoners.diff.config = stn_config;
            solver
        }
    };

    if !skip_model_situ_sat_check {
        // We need to make sure `model` /\ `situ` is indeed SAT.
        match solver.solve_with_assumptions(presupposition.situ.iter().cloned()) {
            Ok(_) => solver.restore(DecLvl::from(presupposition.situ.len())),
            Err(_) => {
                return Err(UnmetPresupposition {
                    presupposition,
                    cause: UnmetPresuppositionCause::ModelSituUnsat,
                })
            }
        }
    } else {
        // If we (the caller of the function) want to skip checking `model` /\ `situ` is SAT
        // (because we know that it's already the case), we only do the initial propagation and assumptions.
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
    }

    // !!! Remember, at this point `situ` is already assumed
    // !!! so, we will just use `query` (or `query_neg`)
    // !!! in `solve_with_assumptions` calls below (incremental solving).
    debug_assert!(solver.current_decision_level() == DecLvl::from(presupposition.situ.len()));

    let res = match presupposition.kind {
        PresuppositionKind::ModelSituUnsatWithQuery => {
            match solver
                .solve_with_assumptions(presupposition.query.iter().cloned())
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
                .solve_with_assumptions(presupposition.query.iter().cloned())
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
                .solve_with_assumptions(presupposition.query.iter().cloned())
                .expect("Solver interrupted.")
            {
                Ok(_) => {
                    solver.restore(dl);
                    let query_neg = presupposition.query.iter().map(|&l| !l);
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
            let query_neg = presupposition.query.iter().map(|&l| !l);
            match solver.solve_with_assumptions(query_neg).expect("Solver interrupted.") {
                Ok(_) => Ok(()),
                Err(_) => Err(UnmetPresupposition {
                    presupposition,
                    cause: UnmetPresuppositionCause::ModelSituNegQueryUnsat,
                }),
            }
        }
    };
    // necessary if the solver was a cached one (given as parameter),
    // to ensure it can be safely reused somewhere else.
    solver.reset();
    res
}
