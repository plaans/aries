use crate::variable::Variable;

use super::Goal;

#[derive(PartialEq, Debug)]
pub struct Objective<'a> {
    pub goal: Goal,
    pub variable: &'a Variable,
}