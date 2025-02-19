use crate::variable::Variable;

use super::Goal;

pub struct Objective<'a> {
    pub goal: Goal,
    pub variable: &'a Variable,
}