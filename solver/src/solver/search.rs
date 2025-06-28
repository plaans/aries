pub mod activity;
pub mod beta;
pub mod combinators;
pub mod conflicts;
pub mod lexical;
pub mod random;

use crate::backtrack::{Backtrack, DecLvl};
use crate::core::state::{Conflict, Explainer};
use crate::core::*;
use crate::model::extensions::SavedAssignment;
use crate::model::{Label, Model};
use crate::solver::stats::Stats;

pub enum Decision {
    SetLiteral(Lit),
    Restart,
}

pub type Brancher<L> = Box<dyn SearchControl<L> + Send>;

pub fn default_brancher<Lbl: Label>() -> Brancher<Lbl> {
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

    /// Notifies the search control that a new assignment has been found (either by itself or by an other solver running in parallel).
    fn new_assignment_found(&mut self, objective_value: IntCst, assignment: std::sync::Arc<SavedAssignment>) {}

    /// Invoked by search immediately before saving the state
    fn pre_save_state(&mut self, _model: &Model<Lbl>) {}

    /// Invoked by search, immediately when a conflict is found.
    /// In particular, it is invoked before analysing the confilct, which might partially undo the trail.
    fn pre_conflict_analysis(&mut self, _model: &Model<Lbl>) {}

    /// Invoked by search when facing a conflict in the search.
    /// Also indicate the level at which the search would backtrack as a result fo this conflict.
    fn conflict(
        &mut self,
        clause: &Conflict,
        model: &Model<Lbl>,
        explainer: &mut dyn Explainer,
        backtrack_level: DecLvl,
    ) {
    }

    fn clone_to_box(&self) -> Brancher<Lbl>;
}
