// ============= Forward progression ===========

use crate::problem::Encoding;
use crate::search::Model;
use crate::Var;
use aries::backtrack::{Backtrack, DecLvl, DecisionLevelTracker};
use aries::core::state::{OptDomain, Term};
use aries::core::{IntCst, Lit, VarRef};
use aries::model::extensions::AssignmentExt;
use aries::solver::search::{Decision, SearchControl};
use aries::solver::stats::Stats;

#[derive(Clone)]
pub struct EstBrancher {
    pb: Encoding,
    lvl: DecisionLevelTracker,
}

impl EstBrancher {
    pub fn new(pb: &Encoding) -> Self {
        EstBrancher {
            pb: pb.clone(),
            lvl: Default::default(),
        }
    }
}

impl SearchControl<Var> for EstBrancher {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        // among the task with the smallest "earliest starting time (est)" pick the one that has the least slack
        let best = active_tasks(&self.pb, model).min_by_key(|(_var, est, lst)| (*est, *lst));

        // decision is to set the start time to the selected task to the smallest possible value.
        // if no task was selected, it means that they are all instantiated and we have a complete schedule
        best.map(|(var, est, _)| {
            let prez = model.presence_literal(var);
            if !model.entails(prez) {
                Decision::SetLiteral(prez)
            } else {
                Decision::SetLiteral(Lit::leq(var, est))
            }
        })
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for EstBrancher {
    fn save_state(&mut self) -> DecLvl {
        self.lvl.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.lvl.num_saved()
    }

    fn restore_last(&mut self) {
        self.lvl.restore_last()
    }
}

/// Returns an iterator over all timepoints that not bound yet.
/// Each item in the iterator is a tuple `(var, est, lst)` where:
///  - `var` is the temporal variable
///  - `est` is its lower bound (the earliest start time of the task)
///  - `lst` is its upper bound (the latest start time of the task)
///  - `est < lst`: the start time of the task has not been decided yet.
fn active_tasks<'a>(pb: &'a Encoding, model: &'a Model) -> impl Iterator<Item = (VarRef, IntCst, IntCst)> + 'a {
    pb.all_alternatives().filter_map(move |a| {
        let v = a.start().var.variable();
        // keep all variables that not absent and not bound
        match model.opt_domain_of(v) {
            OptDomain::Present(lb, ub) if lb < ub => Some((v, lb, ub)),
            OptDomain::Unknown(lb, ub) => Some((v, lb, ub)),
            _ => None,
        }
    })
}
