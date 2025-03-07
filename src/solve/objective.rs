use crate::solve::Goal;
use crate::var::BasicVar;

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