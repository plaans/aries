pub mod activity;

use crate::solver::stats::Stats;
use aries::backtrack::Backtrack;
use aries_core::state::{Conflict, Explainer};
use aries_core::*;
use aries_model::extensions::SavedAssignment;
use aries_model::{Label, Model};

pub enum Decision {
    SetLiteral(Lit),
    Restart,
}

pub fn default_brancher<Lbl: Label>() -> Box<dyn SearchControl<Lbl> + Send> {
    Box::new(activity::ActivityBrancher::new())
}

#[allow(unused_variables)]
pub trait SearchControl<Lbl>: Backtrack {
    /// Select the next decision to make while maintaining the invariant that every non bound variable remains in the queue.
    ///
    /// This invariant allows to invoke this function at the decision level preceding the one of the decision that will be returned.
    /// A nice side-effects is that any variable that is bound and remove from the queue will only be added back if backtracking
    /// to the level preceding the decision to be made.
    ///
    /// Returns `None` if no decision is left to be made.
    fn next_decision(&mut self, stats: &Stats, model: &Model<Lbl>) -> Option<Decision>;

    fn import_vars(&mut self, model: &Model<Lbl>) {}

    /// Notifies the search control that a new assignment has been found (either if itself or by an other solver running in parallel).
    fn new_assignment_found(&mut self, objective_value: IntCst, assignment: std::sync::Arc<SavedAssignment>) {}

    fn pre_save_state(&mut self, _model: &Model<Lbl>) {}
    fn pre_conflict_analysis(&mut self, _model: &Model<Lbl>) {}
    /// Invoked by search when facing a conflict in the search
    fn conflict(&mut self, clause: &Conflict, model: &Model<Lbl>, explainer: &mut dyn Explainer) {}
    fn asserted_after_conflict(&mut self, lit: Lit, model: &Model<Lbl>) {}

    fn clone_to_box(&self) -> Box<dyn SearchControl<Lbl> + Send>;
}
