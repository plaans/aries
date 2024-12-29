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

pub struct UnmetPresupposition<Lbl: Label> {
    presupposition: Presupposition<Lbl>,
    cause: UnmetPresuppositionCause,
}

pub enum PresuppositionKind {
    ModelSituUnsatWithQuery,
    ModelSituSatWithQuery,
    ModelSituNotEntailQuery,
    ModelSituEntailQuery,
}

pub enum UnmetPresuppositionCause {
    ModelSituUnsat,
    ModelSituQueryUnsat,
    ModelSituQuerySat,
    ModelSituNegQueryUnsat,
    ModelSituNegQuerySat,
}

pub fn check_presupposition<Lbl: Label>(presupposition: Presupposition<Lbl>, cached_solver: Option<&mut Solver<Lbl>>) -> Result<(), UnmetPresupposition<Lbl>> {
    let solver = if let Some(s) = cached_solver {
        s
    } else {
        &mut create_solver((*presupposition.model).clone())
    };
    match solver.solve_with_assumptions(presupposition.situ.clone()) {
        Err(_) => return Err(UnmetPresupposition { presupposition, cause: UnmetPresuppositionCause::ModelSituUnsat }),
        Ok(_) => solver.restore(DecLvl::from(presupposition.situ.len()))
    };

    // Remember, `situ` is already assumed (we backtracked to the latest assumption).
    debug_assert!(solver.current_decision_level() == DecLvl::from(presupposition.situ.len()));
    // And so, we will just use `query` (or `query_neg`) in `solve_with_assumptions` calls below (incremental solving).
    
    let res = match presupposition.kind {
        PresuppositionKind::ModelSituUnsatWithQuery => {
            match solver.solve_with_assumptions(presupposition.query.clone()).expect("Solver interrupted.") {
                Ok(_) => Err(UnmetPresupposition { presupposition, cause: UnmetPresuppositionCause::ModelSituQuerySat }),
                Err(_) => Ok(()),
            }
        }
        PresuppositionKind::ModelSituSatWithQuery => {
            match solver.solve_with_assumptions(presupposition.query.clone()).expect("Solver interrupted.") {
                Ok(_) => Ok(()),
                Err(_) => Err(UnmetPresupposition { presupposition, cause: UnmetPresuppositionCause::ModelSituQueryUnsat }),
            }
        },
        PresuppositionKind::ModelSituNotEntailQuery => {
            let dl = DecLvl::from(presupposition.query.len());
            match solver.solve_with_assumptions(presupposition.query.clone()).expect("Solver interrupted.") {
                Ok(_) => {
                    solver.restore(dl);
                    let query_neg = presupposition.query.iter().map(|&l| !l).collect_vec();
                    match solver.solve_with_assumptions(query_neg).expect("Solver interrupted.") {
                        Ok(_) => Err(UnmetPresupposition { presupposition, cause: UnmetPresuppositionCause::ModelSituNegQuerySat }),
                        Err(_) => Ok(()),
                    }        
                },
                Err(_) => Err(UnmetPresupposition { presupposition, cause: UnmetPresuppositionCause::ModelSituQueryUnsat }),
            }
        }
        PresuppositionKind::ModelSituEntailQuery => {
            let neg_query = presupposition.query.iter().map(|&l| !l).collect_vec();
            match solver.solve_with_assumptions(neg_query).expect("Solver interrupted.") {
                Ok(_) => Ok(()),
                Err(_) => Err(UnmetPresupposition { presupposition, cause: UnmetPresuppositionCause::ModelSituNegQueryUnsat }),
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
