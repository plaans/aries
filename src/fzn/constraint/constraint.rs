use crate::fzn::constraint::builtins::*;

#[derive(Clone, Debug)]
pub enum Constraint {
    IntEq(IntEq),
    IntLinEq(IntLinEq),
    IntLinLe(IntLinLe),
    BoolEq(BoolEq),
}
