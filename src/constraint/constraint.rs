use crate::constraint::builtins::*;

#[derive(Clone, Debug)]
pub enum Constraint {
    IntEq(IntEq),
    BoolEq(BoolEq),
}
