use std::ops::Add;

use crate::core::{state::Term, IntCst, VarRef};

/// Represents a eq or neq relationship between two variables.
/// Option\<EqRelation> should be used to represent a relationship between any two vars
///
/// Use + to combine two relationships. eq + neq = Some(neq), neq + neq = None
#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub enum EqRelation {
    Eq,
    Neq,
}

impl Add for EqRelation {
    type Output = Option<Self>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (EqRelation::Eq, EqRelation::Eq) => Some(EqRelation::Eq),
            (EqRelation::Neq, EqRelation::Eq) => Some(EqRelation::Neq),
            (EqRelation::Eq, EqRelation::Neq) => Some(EqRelation::Neq),
            (EqRelation::Neq, EqRelation::Neq) => None,
        }
    }
}

/// A variable or a constant used as a node in the graph
#[derive(Hash, Eq, PartialEq, Copy, Clone, Debug, Ord, PartialOrd)]
pub enum Node {
    Var(VarRef),
    Val(IntCst),
}

impl From<VarRef> for Node {
    fn from(v: VarRef) -> Self {
        Node::Var(v)
    }
}

impl From<IntCst> for Node {
    fn from(v: IntCst) -> Self {
        Node::Val(v)
    }
}

impl TryInto<VarRef> for Node {
    type Error = IntCst;

    fn try_into(self) -> Result<VarRef, Self::Error> {
        match self {
            Node::Var(v) => Ok(v),
            Node::Val(v) => Err(v),
        }
    }
}

impl Term for Node {
    fn variable(self) -> VarRef {
        match self {
            Node::Var(v) => v,
            Node::Val(_) => VarRef::ZERO,
        }
    }
}
