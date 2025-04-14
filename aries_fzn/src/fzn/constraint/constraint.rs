use std::collections::HashMap;

use aries::core::VarRef;

use crate::aries::Post;
use crate::fzn::constraint::builtins::*;
use crate::fzn::constraint::Encode;

#[derive(Clone, Debug)]
pub enum Constraint {
    ArrayIntElement(ArrayIntElement),
    ArrayIntMaximum(ArrayIntMaximum),
    ArrayIntMinimum(ArrayIntMinimum),
    ArrayVarIntElement(ArrayVarIntElement),
    IntAbs(IntAbs),
    IntEq(IntEq),
    IntEqReif(IntEqReif),
    IntLe(IntLe),
    IntLinEq(IntLinEq),
    IntLinEqImp(IntLinEqImp),
    IntLinLe(IntLinLe),
    IntLinLeImp(IntLinLeImp),
    IntLinNe(IntLinNe),
    IntNe(IntNe),
    ArrayBoolAnd(ArrayBoolAnd),
    Bool2Int(Bool2Int),
    BoolClause(BoolClause),
    BoolEq(BoolEq),
}

impl Encode for Constraint {
    fn encode(
        &self,
        translation: &HashMap<usize, VarRef>,
    ) -> Box<dyn Post<usize>> {
        match self {
            Constraint::ArrayIntElement(c) => c.encode(translation),
            Constraint::ArrayIntMaximum(c) => c.encode(translation),
            Constraint::ArrayIntMinimum(c) => c.encode(translation),
            Constraint::ArrayVarIntElement(c) => c.encode(translation),
            Constraint::IntAbs(c) => c.encode(translation),
            Constraint::IntEq(c) => c.encode(translation),
            Constraint::IntEqReif(c) => c.encode(translation),
            Constraint::IntLe(c) => c.encode(translation),
            Constraint::IntLinEq(c) => c.encode(translation),
            Constraint::IntLinEqImp(c) => c.encode(translation),
            Constraint::IntLinLe(c) => c.encode(translation),
            Constraint::IntLinLeImp(c) => c.encode(translation),
            Constraint::IntLinNe(c) => c.encode(translation),
            Constraint::IntNe(c) => c.encode(translation),
            Constraint::ArrayBoolAnd(c) => c.encode(translation),
            Constraint::Bool2Int(c) => c.encode(translation),
            Constraint::BoolClause(c) => c.encode(translation),
            Constraint::BoolEq(c) => c.encode(translation),
        }
    }
}
