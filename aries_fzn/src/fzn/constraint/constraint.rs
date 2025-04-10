use std::collections::HashMap;

use aries::core::VarRef;

use crate::aries::Post;
use crate::fzn::constraint::builtins::*;
use crate::fzn::constraint::Encode;

/// A flatzinc constraint.
///
/// ```flatzinc
/// constraint int_le(x,y);
/// ```
#[derive(Clone, Debug)]
pub enum Constraint {
    ArrayIntMaximum(ArrayIntMaximum),
    ArrayIntMinimum(ArrayIntMinimum),
    IntAbs(IntAbs),
    IntEq(IntEq),
    IntEqReif(IntEqReif),
    IntLe(IntLe),
    IntLinEq(IntLinEq),
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
            Constraint::ArrayIntMaximum(c) => c.encode(translation),
            Constraint::ArrayIntMinimum(c) => c.encode(translation),
            Constraint::IntAbs(c) => c.encode(translation),
            Constraint::IntEq(c) => c.encode(translation),
            Constraint::IntEqReif(c) => c.encode(translation),
            Constraint::IntLe(c) => c.encode(translation),
            Constraint::IntLinEq(c) => c.encode(translation),
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
