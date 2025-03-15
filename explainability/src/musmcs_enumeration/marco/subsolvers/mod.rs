mod mapsolver;
mod subsetsolver;

pub(crate) use mapsolver::MapSolver;
pub(crate) use subsetsolver::SubsetSolver;
pub use subsetsolver::SubsetSolverImpl;

use std::collections::BTreeSet;

use aries::{backtrack::Backtrack, core::Lit, model::{Label, Model}, solver::{Exit, Solver, UnsatCore}};

#[allow(dead_code)]
pub(crate) struct SimpleSubsetSolverImpl<Lbl: Label> {
    solver: Solver<Lbl>,
}
#[allow(dead_code)]
impl<Lbl: Label> SimpleSubsetSolverImpl<Lbl> {
    pub fn new(model: Model<Lbl>) -> Self {
        Self { solver: Solver::new(model) }
    }
}
impl<Lbl: Label> SubsetSolverImpl<Lbl> for SimpleSubsetSolverImpl<Lbl> {
    fn get_model(&mut self) -> &mut Model<Lbl> {
        &mut self.solver.model
    }
    fn find_unsat_core(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit> {
        if let Err((_, unsat_core)) = { self.solver.incremental_push_all(subset.iter().copied()) } {
            self.solver.reset();
            Ok(Err(unsat_core))
        } else {
            let res = self.solver.incremental_solve().map(|r| r.map(|_| ()));
            self.solver.reset_search();
            res
        }
    }
}
