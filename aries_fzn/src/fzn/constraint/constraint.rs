use std::collections::HashMap;

use aries::core::VarRef;

use crate::aries::Post;
use crate::fzn::constraint::Encode;
use crate::fzn::constraint::builtins::*;

#[derive(Clone, Debug)]
pub enum Constraint {
    ArrayIntElement(ArrayIntElement),
    ArrayIntMaximum(ArrayIntMaximum),
    ArrayIntMinimum(ArrayIntMinimum),
    ArrayVarIntElement(ArrayVarIntElement),
    IntAbs(IntAbs),
    IntEq(IntEq),
    IntEqImp(IntEqImp),
    IntEqReif(IntEqReif),
    IntLe(IntLe),
    IntLeImp(IntLeImp),
    IntLeReif(IntLeReif),
    IntLinEq(IntLinEq),
    IntLinEqImp(IntLinEqImp),
    IntLinEqReif(IntLinEqReif),
    IntLinLe(IntLinLe),
    IntLinLeImp(IntLinLeImp),
    IntLinLeReif(IntLinLeReif),
    IntLinNe(IntLinNe),
    IntLinNeReif(IntLinNeReif),
    IntLt(IntLt),
    IntLtReif(IntLtReif),
    IntNe(IntNe),
    IntNeReif(IntNeReif),
    IntTimes(IntTimes),
    ArrayBoolAnd(ArrayBoolAnd),
    ArrayBoolElement(ArrayBoolElement),
    ArrayVarBoolElement(ArrayVarBoolElement),
    Bool2Int(Bool2Int),
    BoolClause(BoolClause),
    BoolClauseReif(BoolClauseReif),
    BoolEq(BoolEq),
    BoolEqImp(BoolEqImp),
    BoolEqReif(BoolEqReif),
    BoolLe(BoolLe),
    BoolLeReif(BoolLeReif),
    BoolLinEq(BoolLinEq),
    BoolLinLe(BoolLinLe),
    BoolNot(BoolNot),
    BoolXor(BoolXor),
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
            Constraint::IntEqImp(c) => c.encode(translation),
            Constraint::IntEqReif(c) => c.encode(translation),
            Constraint::IntLe(c) => c.encode(translation),
            Constraint::IntLeImp(c) => c.encode(translation),
            Constraint::IntLeReif(c) => c.encode(translation),
            Constraint::IntLinEq(c) => c.encode(translation),
            Constraint::IntLinEqImp(c) => c.encode(translation),
            Constraint::IntLinEqReif(c) => c.encode(translation),
            Constraint::IntLinLe(c) => c.encode(translation),
            Constraint::IntLinLeImp(c) => c.encode(translation),
            Constraint::IntLinLeReif(c) => c.encode(translation),
            Constraint::IntLinNe(c) => c.encode(translation),
            Constraint::IntLinNeReif(c) => c.encode(translation),
            Constraint::IntLt(c) => c.encode(translation),
            Constraint::IntLtReif(c) => c.encode(translation),
            Constraint::IntNe(c) => c.encode(translation),
            Constraint::IntNeReif(c) => c.encode(translation),
            Constraint::IntTimes(c) => c.encode(translation),
            Constraint::ArrayBoolAnd(c) => c.encode(translation),
            Constraint::ArrayBoolElement(c) => c.encode(translation),
            Constraint::ArrayVarBoolElement(c) => c.encode(translation),
            Constraint::Bool2Int(c) => c.encode(translation),
            Constraint::BoolClause(c) => c.encode(translation),
            Constraint::BoolClauseReif(c) => c.encode(translation),
            Constraint::BoolEq(c) => c.encode(translation),
            Constraint::BoolEqImp(c) => c.encode(translation),
            Constraint::BoolEqReif(c) => c.encode(translation),
            Constraint::BoolLe(c) => c.encode(translation),
            Constraint::BoolLeReif(c) => c.encode(translation),
            Constraint::BoolLinEq(c) => c.encode(translation),
            Constraint::BoolLinLe(c) => c.encode(translation),
            Constraint::BoolNot(c) => c.encode(translation),
            Constraint::BoolXor(c) => c.encode(translation),
        }
    }
}
