use std::fmt::Display;

use crate::{
    core::Lit,
    reasoners::eq_alt::{
        graph::Edge,
        node::Node,
        propagators::{Enabler, Propagator},
    },
};

/// Edge label used for generic type Edge in DirEqGraph
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct EdgeLabel {
    pub l: Lit,
}

/// A propagator is essentially the same as an edge, except an edge is necessarily valid
/// since it has been added to the graph
impl From<Propagator> for Edge<Node, EdgeLabel> {
    fn from(
        Propagator {
            a,
            b,
            relation,
            enabler: Enabler { active, .. },
        }: Propagator,
    ) -> Self {
        Self::new(a, b, EdgeLabel { l: active }, relation)
    }
}

impl Display for EdgeLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.l)
    }
}
