mod mapsolver;
mod subsetsolver;

use itertools::Itertools;
pub(crate) use mapsolver::MapSolver;
pub(crate) use subsetsolver::SubsetSolver;
pub use subsetsolver::SubsetSolverImpl;

use std::collections::BTreeSet;

use aries::backtrack::{Backtrack, DecLvl};
use aries::core::Lit;
use aries::model::{lang::expr::or, Label, Model};
use aries::solver::{Exit, Solver, UnsatCore};

#[allow(dead_code)]
pub(crate) struct SimpleSubsetSolverImpl<Lbl: Label> {
    solver: Solver<Lbl>,
}
#[allow(dead_code)]
impl<Lbl: Label> SimpleSubsetSolverImpl<Lbl> {
    pub fn new(model: Model<Lbl>) -> Self {
        Self {
            solver: Solver::new(model),
        }
    }
}
impl<Lbl: Label> SubsetSolverImpl<Lbl> for SimpleSubsetSolverImpl<Lbl> {
    fn get_model(&mut self) -> &mut Model<Lbl> {
        &mut self.solver.model
    }
    fn find_unsat_core(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit> {
        for (i, a) in self.solver.model.state.assumptions().iter().enumerate() {
            if !subset.contains(a) {
                self.solver.restore(DecLvl::new(u32::try_from(i).unwrap()));
                break;
            }
        }
        let res = self
            .solver
            .incremental_push_all(subset.iter().copied().collect_vec())
            .map_or_else(|(_, uc)| Ok(Err(uc)), |_| self.solver.incremental_solve())?;
        if let Err(unsat_core) = res {
            self.solver.reset();
            self.solver.enforce(
                or(unsat_core
                    .literals()
                    .iter()
                    .map(|&l| !l)
                    .collect_vec()
                    .into_boxed_slice()),
                [],
            );
            Ok(Err(unsat_core))
        } else {
            self.solver.reset_search();
            Ok(Ok(()))
        }
    }
}
