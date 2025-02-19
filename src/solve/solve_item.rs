use crate::variable::Variable;

use super::Objective;

pub enum SolveItem<'a> {
    Satisfy,
    Optimize(Objective<'a>),
}

impl<'a> SolveItem<'a> {
    
    /// Return the objective variable if available.
    pub fn variable(&self) -> Option<&Variable> {
        match self {
            SolveItem::Satisfy => None,
            SolveItem::Optimize(Objective { goal: _, variable }) => Some(variable),
        }
    }
}