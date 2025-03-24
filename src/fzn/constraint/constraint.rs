use std::collections::HashMap;

use aries::core::VarRef;

use crate::aries::Post;
use crate::fzn::constraint::builtins::*;
use crate::fzn::constraint::Encode;

#[derive(Clone, Debug)]
pub enum Constraint {
    ArrayIntMaximum(ArrayIntMaximum),
    ArrayIntMinimum(ArrayIntMinimum),
    IntAbs(IntAbs),
    IntEq(IntEq),
    IntLe(IntLe),
    IntLinEq(IntLinEq),
    IntLinLe(IntLinLe),
    BoolEq(BoolEq),
}

impl Encode for Constraint {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        match self {
            Constraint::ArrayIntMaximum(c) => c.encode(translation),
            Constraint::ArrayIntMinimum(c) => c.encode(translation),
            Constraint::IntAbs(c) => c.encode(translation),
            Constraint::IntEq(c) => c.encode(translation),
            Constraint::IntLe(c) => c.encode(translation),
            Constraint::IntLinEq(c) => c.encode(translation),
            Constraint::IntLinLe(c) => c.encode(translation),
            Constraint::BoolEq(c) => c.encode(translation),
        }
    }
}
