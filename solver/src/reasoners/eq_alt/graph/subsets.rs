use crate::core::state::DomainsSnapshot;

use super::{
    traversal::{self},
    EqAdjList, IdEdge, NodeId,
};

impl traversal::Graph for &EqAdjList {
    fn edges(&self, node: NodeId) -> impl Iterator<Item = IdEdge> {
        self.get_edges(node).into_iter().flat_map(|v| v.iter().cloned())
    }
}

/// Subset of `graph` which only contains edges that are active in model.
pub struct ActiveGraphSnapshot<'a, G: traversal::Graph> {
    model: &'a DomainsSnapshot<'a>,
    graph: G,
}

impl<'a, G: traversal::Graph> ActiveGraphSnapshot<'a, G> {
    pub fn new(model: &'a DomainsSnapshot<'a>, graph: G) -> Self {
        Self { model, graph }
    }
}

impl<G: traversal::Graph> traversal::Graph for ActiveGraphSnapshot<'_, G> {
    fn edges(&self, node: NodeId) -> impl Iterator<Item = IdEdge> {
        self.graph.edges(node).filter(|e| self.model.entails(e.active))
    }
}
