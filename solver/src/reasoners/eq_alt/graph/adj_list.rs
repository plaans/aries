use std::fmt::{Debug, Formatter};

use crate::collections::ref_store::IterableRefMap;

use super::{Edge, NodeId};

#[derive(Default, Clone)]
pub struct EqAdjList(IterableRefMap<NodeId, Vec<Edge>>);

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
    pub fn insert_edge(&mut self, edge: Edge) -> bool {
        self.insert_node(edge.source);
        self.insert_node(edge.target);
        let edges = self.get_edges_mut(edge.source).unwrap();
        if edges.contains(&edge) {
            false
        } else {
            edges.push(edge);
            true
        }
    }

    pub fn contains_edge(&self, edge: Edge) -> bool {
        let Some(edges) = self.0.get(edge.source) else {
            return false;
        };
        edges.contains(&edge)
    }

    pub fn iter_edges(&self, node: NodeId) -> impl Iterator<Item = &Edge> {
        self.0.get(node).into_iter().flat_map(|v| v.iter())
    }

    pub fn get_edges_mut(&mut self, node: NodeId) -> Option<&mut Vec<Edge>> {
        self.0.get_mut(node)
    }

    pub fn iter_all_edges(&self) -> impl Iterator<Item = Edge> + use<'_> {
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
        filter: fn(&Edge) -> bool,
    ) -> Option<impl Iterator<Item = NodeId> + use<'_>> {
        self.0
            .get(node)
            .map(move |v| v.iter().filter(move |e| filter(e)).map(|e| e.target))
    }

    pub fn remove_edge(&mut self, edge: Edge) {
        if let Some(set) = self.0.get_mut(edge.source) {
            set.retain(|e| *e != edge)
        }
    }
}
