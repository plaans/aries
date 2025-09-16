use std::fmt::Display;

use crate::core::{
    state::{Domains, DomainsSnapshot, IntDomain, Term},
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

impl TryFrom<Node> for VarRef {
    type Error = ();

    fn try_from(value: Node) -> Result<Self, Self::Error> {
        match value {
            Node::Var(v) => Ok(v),
            Node::Val(_) => Err(()),
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
    pub fn node_domain(&self, n: &Node) -> IntDomain {
        match *n {
            Node::Var(var) => self.int_domain(var),
            Node::Val(cst) => IntDomain::new(cst, cst),
        }
    }
}

impl DomainsSnapshot<'_> {
    pub fn node_domain(&self, n: &Node) -> IntDomain {
        match *n {
            Node::Var(var) => self.int_domain(var),
            Node::Val(cst) => IntDomain::new(cst, cst),
        }
    }
}
