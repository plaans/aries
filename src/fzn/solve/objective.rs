use crate::fzn::solve::Goal;
use crate::fzn::var::BasicVar;

/// Flatzinc optimization objective.
///
/// Minimize or maximize a basic variable.
///
/// ```flatzinc
/// solve minimize stress;
/// ```
#[derive(PartialEq, Debug)]
pub struct Objective {
    goal: Goal,
    variable: BasicVar,
}

impl Objective {
    /// Create a new `Objective`.
    pub fn new(goal: Goal, variable: BasicVar) -> Self {
        Objective { goal, variable }
    }

    /// Return the objective goal.
    pub fn goal(&self) -> &Goal {
        &self.goal
    }

    /// Return the objective variable.
    pub fn variable(&self) -> &BasicVar {
        &self.variable
    }
}
