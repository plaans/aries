use crate::constraint::builtins::BoolAnd;
use crate::constraint::builtins::IntEq;

#[derive(Clone, Debug)]
pub enum Constraint {
    BoolAnd(BoolAnd),
    IntEq(IntEq),
}

impl From<BoolAnd> for Constraint {
    fn from(value: BoolAnd) -> Self {
        Self::BoolAnd(value)
    }
}

impl From<IntEq> for Constraint {
    fn from(value: IntEq) -> Self {
        Self::IntEq(value)
    }
}