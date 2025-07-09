use std::{fmt::Display, ops::Add};

use crate::core::{
    state::{Domains, DomainsSnapshot, Term},
    IntCst, VarRef,
};

/// Represents a eq or neq relationship between two variables.
/// Option\<EqRelation> should be used to represent a relationship between any two vars
///
/// Use + to combine two relationships. eq + neq = Some(neq), neq + neq = None
#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub enum EqRelation {
    Eq,
    Neq,
}

impl Display for EqRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                EqRelation::Eq => "==",
                EqRelation::Neq => "!=",
            }
        )
    }
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

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Var(v) => write!(f, "{:?}", v),
            Node::Val(v) => write!(f, "{}", v),
        }
    }
}

impl Node {
    pub fn get_bound(&self, model: &Domains) -> Option<IntCst> {
        match *self {
            Node::Var(v) => model.get_bound(v),
            Node::Val(v) => Some(v),
        }
    }

    pub fn get_bound_snap(&self, model: &DomainsSnapshot) -> Option<IntCst> {
        match *self {
            Node::Var(v) => model.get_bound(v),
            Node::Val(v) => Some(v),
        }
    }

    pub fn get_bounds(&self, model: &Domains) -> (IntCst, IntCst) {
        match *self {
            Node::Var(v) => model.bounds(v),
            Node::Val(v) => (v, v),
        }
    }

    pub fn get_bounds_snap(&self, model: &DomainsSnapshot) -> (IntCst, IntCst) {
        match *self {
            Node::Var(v) => model.bounds(v),
            Node::Val(v) => (v, v),
        }
    }
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
