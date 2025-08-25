use std::fmt::{Debug, Formatter};

use hashbrown::HashSet;

use crate::collections::ref_store::IterableRefMap;

use super::{IdEdge, NodeId};

#[derive(Default, Clone)]
pub(super) struct EqAdjList(IterableRefMap<NodeId, HashSet<IdEdge>>);

impl Debug for EqAdjList {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f)?;
        for (node, edges) in self.0.entries() {
            if !edges.is_empty() {
                writeln!(f, "{:?}:", node)?;
                for edge in edges {
                    writeln!(f, "  -> {:?}    {:?}", edge.target, edge)?;
                }
            }
        }
        Ok(())
    }
}

#[allow(unused)]
impl EqAdjList {
    pub(super) fn new() -> Self {
        Self(Default::default())
    }

    /// Insert a node if not present
    fn insert_node(&mut self, node: NodeId) {
        if !self.0.contains(node) {
            self.0.insert(node, Default::default());
        }
    }

    /// Possibly insert an edge and both nodes
    /// Returns true if edge was inserted
    pub fn insert_edge(&mut self, edge: IdEdge) -> bool {
        self.insert_node(edge.source);
        self.insert_node(edge.target);
        let edges = self.get_edges_mut(edge.source).unwrap();
        edges.insert(edge)
    }

    pub fn contains_edge(&self, edge: IdEdge) -> bool {
        let Some(edges) = self.0.get(edge.source) else {
            return false;
        };
        edges.contains(&edge)
    }

    pub fn get_edges(&self, node: NodeId) -> Option<&HashSet<IdEdge>> {
        self.0.get(node)
    }

    pub fn iter_edges(&self, node: NodeId) -> impl Iterator<Item = &IdEdge> {
        self.0.get(node).into_iter().flat_map(|v| v.iter())
    }

    pub fn get_edges_mut(&mut self, node: NodeId) -> Option<&mut HashSet<IdEdge>> {
        self.0.get_mut(node)
    }

    pub fn iter_all_edges(&self) -> impl Iterator<Item = IdEdge> + use<'_> {
        self.0.entries().flat_map(|(_, e)| e.iter().cloned())
    }

    pub fn iter_children(&self, node: NodeId) -> Option<impl Iterator<Item = NodeId> + use<'_>> {
        self.0.get(node).map(|v| v.iter().map(|e| e.target))
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = NodeId> + use<'_> {
        self.0.entries().map(|(n, _)| n)
    }

    pub fn iter_nodes_where(
        &self,
        node: NodeId,
        filter: fn(&IdEdge) -> bool,
    ) -> Option<impl Iterator<Item = NodeId> + use<'_>> {
        self.0
            .get(node)
            .map(move |v| v.iter().filter(move |e| filter(e)).map(|e| e.target))
    }

    pub fn remove_edge(&mut self, edge: IdEdge) -> bool {
        self.0.get_mut(edge.source).is_some_and(|set| set.remove(&edge))
    }
}
