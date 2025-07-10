use std::fmt::Display;

use crate::core::{
    state::{Domains, DomainsSnapshot, Term},
    IntCst, VarRef,
};

/// A variable or a constant used as a node in the eq graph
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

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Var(v) => write!(f, "{:?}", v),
            Node::Val(v) => write!(f, "{}", v),
        }
    }
}

impl Domains {
    pub fn get_node_bound(&self, n: &Node) -> Option<IntCst> {
        match *n {
            Node::Var(v) => self.get_bound(v),
            Node::Val(v) => Some(v),
        }
    }

    pub fn get_node_bounds(&self, n: &Node) -> (IntCst, IntCst) {
        match *n {
            Node::Var(v) => self.bounds(v),
            Node::Val(v) => (v, v),
        }
    }
}

impl DomainsSnapshot<'_> {
    pub fn get_node_bound(&self, n: &Node) -> Option<IntCst> {
        match *n {
            Node::Var(v) => self.get_bound(v),
            Node::Val(v) => Some(v),
        }
    }

    pub fn get_node_bounds(&self, n: &Node) -> (IntCst, IntCst) {
        match *n {
            Node::Var(v) => self.bounds(v),
            Node::Val(v) => (v, v),
        }
    }
}
