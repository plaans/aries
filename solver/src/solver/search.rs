mod activity;

use crate::solver::stats::Stats;
use aries_backtrack::{Backtrack, DecLvl};
use aries_model::bounds::Bound;
use aries_model::lang::{IntCst, VarRef};
use aries_model::Model;

pub enum Decision {
    SetLiteral(Bound),
    Restart,
}

pub trait SearchControl: Backtrack {
    fn import_vars(&mut self, model: &Model);

    /// Select the next decision to make while maintaining the invariant that every non bound variable remains in the queue.
    ///
    /// This invariant allows to invoke this function at the decision level preceding the one of the decision that will be returned.
    /// A nice side-effects is that any variable that is bound and remove from the queue will only be added back if backtracking
    /// to the level preceding the decision to be made.
    ///
    /// Returns `None` if no decision is left to be made.
    fn next_decision(&mut self, stats: &Stats, model: &Model) -> Option<Decision>;

    fn set_default_value(&mut self, var: VarRef, val: IntCst);

    fn set_default_values_from(&mut self, assignment: &Model);

    /// Increase the activity of the variable and perform an reordering in the queue.
    /// The activity is then used to select the next variable.
    fn bump_activity(&mut self, bvar: VarRef);

    fn decay_activities(&mut self);
}

pub struct Brancher(Box<dyn SearchControl>);

impl Brancher {
    pub fn import_vars(&mut self, model: &Model) {
        self.0.import_vars(model)
    }

    /// Select the next decision to make while maintaining the invariant that every non bound variable remains in the queue.
    ///
    /// This invariant allows to invoke this function at the decision level preceding the one of the decision that will be returned.
    /// A nice side-effects is that any variable that is bound and remove from the queue will only be added back if backtracking
    /// to the level preceding the decision to be made.
    ///
    /// Returns `None` if no decision is left to be made.
    pub fn next_decision(&mut self, stats: &Stats, model: &Model) -> Option<Decision> {
        self.0.next_decision(stats, model)
    }

    pub fn set_default_value(&mut self, var: VarRef, val: IntCst) {
        self.0.set_default_value(var, val)
    }

    pub fn set_default_values_from(&mut self, assignment: &Model) {
        self.0.set_default_values_from(assignment)
    }

    /// Increase the activity of the variable and perform an reordering in the queue.
    /// The activity is then used to select the next variable.
    pub fn bump_activity(&mut self, bvar: VarRef) {
        self.0.bump_activity(bvar)
    }

    pub fn decay_activities(&mut self) {
        self.0.decay_activities()
    }
}

impl Backtrack for Brancher {
    fn save_state(&mut self) -> DecLvl {
        self.0.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.0.num_saved()
    }

    fn restore_last(&mut self) {
        self.0.restore_last()
    }
}

impl<T: 'static + SearchControl + Backtrack> From<T> for Brancher {
    fn from(x: T) -> Self {
        Brancher(Box::new(x))
    }
}

impl Default for Brancher {
    fn default() -> Self {
        activity::ActivityBrancher::new().into()
    }
}
