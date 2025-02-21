use crate::solve::Goal;
use crate::variable::SharedVariable;

#[derive(PartialEq, Debug)]
pub struct Objective {
    goal: Goal,
    variable: SharedVariable,
}

impl Objective {
    /// Create a new `Objective`.
    pub fn new(goal: Goal, variable: SharedVariable) -> Self {
        Objective { goal, variable }
    }

    /// Return the objective goal.
    pub fn goal(&self) -> &Goal {
        &self.goal
    }

    /// Return the objective variable.
    pub fn variable(&self) -> &SharedVariable {
        &self.variable
    }
}