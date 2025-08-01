use itertools::Itertools;

use crate::core::state::DomainsSnapshot;

use super::{
    node_store::NodeStore,
    traversal::{self},
    EqAdjList, IdEdge, NodeId,
};

impl traversal::Graph for &EqAdjList {
    fn edges(&self, node: NodeId) -> impl Iterator<Item = IdEdge> {
        self.get_edges(node).into_iter().flat_map(|v| v.clone())
    }

    fn map_source(&self, node: NodeId) -> NodeId {
        node
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

    fn map_source(&self, node: NodeId) -> NodeId {
        self.graph.map_source(node)
    }
}

/// Representation of `graph` which works on group representatives instead of nodes
pub struct MergedGraph<'a, G: traversal::Graph> {
    node_store: &'a NodeStore,
    graph: G,
}

// INVARIANT: All NodeIds returned (also in IdEdge) should be GroupIds
impl<'a, G: traversal::Graph> traversal::Graph for MergedGraph<'a, G> {
    fn map_source(&self, node: NodeId) -> NodeId {
        // INVARIANT: return value is converted from GroupId
        self.node_store.get_representative(self.graph.map_source(node)).into()
    }

    fn edges(&self, node: NodeId) -> impl Iterator<Item = IdEdge> {
        debug_assert_eq!(node, self.node_store.get_representative(node).into());
        let nodes: Vec<NodeId> = self.node_store.get_group(node.into());
        let mut res = Vec::new();
        // INVARIANT: Every value pushed to res has node (a GroupId guaranteed by assertion) as a source
        // and a value converted from GroupId as a target
        for n in nodes {
            res.extend(self.graph.edges(n).map(|e| IdEdge {
                source: node,
                target: self.node_store.get_representative(e.target).into(),
                ..e
            }));
        }
        res.into_iter().unique()
    }
}

impl<'a, G: traversal::Graph> MergedGraph<'a, G> {
    pub fn new(node_store: &'a NodeStore, graph: G) -> Self {
        Self { node_store, graph }
    }
}
