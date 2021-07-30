pub mod activity;

use crate::solver::stats::Stats;
use aries_backtrack::Backtrack;
use aries_model::bounds::Bound;
use aries_model::lang::{IntCst, VarRef};
use aries_model::Model;

pub enum Decision {
    SetLiteral(Bound),
    Restart,
}

pub fn default_brancher() -> Box<dyn SearchControl + Send> {
    Box::new(activity::ActivityBrancher::new())
}

#[allow(unused_variables)]
pub trait SearchControl: Backtrack {
    /// Select the next decision to make while maintaining the invariant that every non bound variable remains in the queue.
    ///
    /// This invariant allows to invoke this function at the decision level preceding the one of the decision that will be returned.
    /// A nice side-effects is that any variable that is bound and remove from the queue will only be added back if backtracking
    /// to the level preceding the decision to be made.
    ///
    /// Returns `None` if no decision is left to be made.
    fn next_decision(&mut self, stats: &Stats, model: &Model) -> Option<Decision>;

    fn import_vars(&mut self, model: &Model) {}

    fn set_default_value(&mut self, var: VarRef, val: IntCst) {}

    fn set_default_values_from(&mut self, assignment: &Model) {}

    /// Increase the activity of the variable and perform an reordering in the queue.
    /// The activity is then used to select the next variable.
    fn bump_activity(&mut self, bvar: VarRef) {}

    fn decay_activities(&mut self) {}

    fn clone_to_box(&self) -> Box<dyn SearchControl + Send>;
}