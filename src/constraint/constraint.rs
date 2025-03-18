use crate::constraint::builtins::*;

#[derive(Clone, Debug)]
pub enum Constraint {
    IntEq(IntEq),
    IntLinEq(IntLinEq),
    BoolEq(BoolEq),
}
