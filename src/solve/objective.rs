use crate::solve::Goal;
use crate::variable::BasicVariable;

#[derive(PartialEq, Debug)]
pub struct Objective {
    goal: Goal,
    variable: BasicVariable,
}

impl Objective {
    /// Create a new `Objective`.
    pub fn new(goal: Goal, variable: BasicVariable) -> Self {
        Objective { goal, variable }
    }

    /// Return the objective goal.
    pub fn goal(&self) -> &Goal {
        &self.goal
    }

    /// Return the objective variable.
    pub fn variable(&self) -> &BasicVariable {
        &self.variable
    }
}