// ============= Forward progression ===========

use crate::problem::{Op, Problem};
use crate::search::Model;
use crate::Var;
use aries_backtrack::{Backtrack, DecLvl};
use aries_core::{IntCst, Lit, VarRef};
use aries_model::extensions::AssignmentExt;
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;

#[derive(Clone)]
pub struct EstBrancher {
    pb: Problem,
    saved: DecLvl,
}

impl EstBrancher {
    pub fn new(pb: &Problem) -> Self {
        EstBrancher {
            pb: pb.clone(),
            saved: DecLvl::ROOT,
        }
    }
}

impl SearchControl<Var> for EstBrancher {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        // among the task with the smallest "earliest starting time (est)" pick the one that has the least slack
        let best = active_tasks(&self.pb, model).min_by_key(|(_var, est, lst)| (*est, *lst));

        // decision is to set the start time to the selected task to the smallest possible value.
        // if no task was selected, it means that they are all instantiated and we have a complete schedule
        best.map(|(var, est, _)| Decision::SetLiteral(Lit::leq(var, est)))
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for EstBrancher {
    fn save_state(&mut self) -> DecLvl {
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.saved -= 1;
    }
}

/// Returns an iterator over all timepoints that not bound yet.
/// Each item in the iterator is a tuple `(var, est, lst)` where:
///  - `var` is the temporal variable
///  - `est` is its lower bound (the earliest start time of the task)
///  - `lst` is its upper bound (the latest start time of the task)
///  - `est < lst`: the start time of the task has not been decided yet.
fn active_tasks<'a>(pb: &'a Problem, model: &'a Model) -> impl Iterator<Item = (VarRef, IntCst, IntCst)> + 'a {
    pb.operations()
        .iter()
        .copied()
        .filter_map(move |Op { job, op_id, .. }| {
            let v = model.shape.get_variable(&Var::Start(job, op_id)).unwrap();
            let (lb, ub) = model.domain_of(v);
            if lb < ub {
                Some((v, lb, ub))
            } else {
                None
            }
        })
}
