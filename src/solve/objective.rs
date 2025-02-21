use crate::solve::Goal;
use crate::variable::Variable;

#[derive(PartialEq, Debug)]
pub struct Objective {
    goal: Goal,
    variable: Variable,
}

impl Objective {
    /// Create a new `Objective`.
    pub fn new(goal: Goal, variable: Variable) -> Self {
        Objective { goal, variable }
    }

    /// Return the objective goal.
    pub fn goal(&self) -> &Goal {
        &self.goal
    }

    /// Return the objective variable.
    pub fn variable(&self) -> &Variable {
        &self.variable
    }
}