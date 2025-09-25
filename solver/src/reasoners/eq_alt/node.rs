use std::fmt::Display;

use crate::core::{
    state::{Domains, DomainsSnapshot, IntDomain, Term},
    IntCst, Lit, VarRef,
};

/// A variable or a constant used as a node in the eq graph
#[derive(Hash, Eq, PartialEq, Copy, Clone, Debug, Ord, PartialOrd)]
pub enum Node {
    Var(VarRef),
    Val(IntCst),
}

impl Node {
    /// Returns false is self == other is impossible according to the model
    pub fn can_be_eq(&self, other: &Node, model: &impl NodeDomains) -> bool {
        !model.node_domain(self).disjoint(&model.node_domain(other))
    }

    /// Returns false is self != other is impossible according to the model
    pub fn can_be_neq(&self, other: &Node, model: &impl NodeDomains) -> bool {
        !model
            .node_domain(self)
            .as_singleton()
            .is_some_and(|bound| model.node_domain(other).is_bound_to(bound))
    }

    pub fn ub_literal(&self, model: &DomainsSnapshot) -> Option<Lit> {
        if let Node::Var(v) = self {
            Some(v.leq(model.ub(*v)))
        } else {
            None
        }
    }

    pub fn lb_literal(&self, model: &DomainsSnapshot) -> Option<Lit> {
        if let Node::Var(v) = self {
            Some(v.geq(model.lb(*v)))
        } else {
            None
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

pub trait NodeDomains {
    fn node_domain(&self, n: &Node) -> IntDomain;
}

impl NodeDomains for Domains {
    fn node_domain(&self, n: &Node) -> IntDomain {
        match *n {
            Node::Var(var) => self.int_domain(var),
            Node::Val(cst) => IntDomain::new(cst, cst),
        }
    }
}

impl NodeDomains for DomainsSnapshot<'_> {
    fn node_domain(&self, n: &Node) -> IntDomain {
        match *n {
            Node::Var(var) => self.int_domain(var),
            Node::Val(cst) => IntDomain::new(cst, cst),
        }
    }
}
