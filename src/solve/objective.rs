use crate::variable::Variable;

use super::Goal;

pub struct Objective {
    pub goal: Goal,
    pub variable: Variable,
}