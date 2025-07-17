use crate::reasoners::eq_alt::{
    graph::Edge,
    node::Node,
    propagators::{Enabler, Propagator},
};

/// A propagator is essentially the same as an edge, except an edge is necessarily valid
/// since it has been added to the graph
impl From<Propagator> for Edge<Node> {
    fn from(
        Propagator {
            a,
            b,
            relation,
            enabler: Enabler { active, .. },
        }: Propagator,
    ) -> Self {
        Self::new(a, b, active, relation)
    }
}
