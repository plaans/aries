use std::fmt::{Debug, Formatter};

use crate::collections::ref_store::IterableRefMap;

use super::{IdEdge, NodeId};

#[derive(Default, Clone)]
pub(super) struct EqAdjList(IterableRefMap<NodeId, Vec<IdEdge>>);

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
    pub(super) fn insert_node(&mut self, node: NodeId) {
        if !self.0.contains(node) {
            self.0.insert(node, Default::default());
        }
    }

    /// Insert an edge and possibly a node
    /// First return val is if source node was inserted, second is if target val was inserted, third is if edge was inserted
    pub(super) fn insert_edge(&mut self, edge: IdEdge) {
        self.insert_node(edge.source);
        self.insert_node(edge.target);
        let edges = self.get_edges_mut(edge.source).unwrap();
        if !edges.contains(&edge) {
            edges.push(edge);
        }
    }

    pub fn contains_edge(&self, edge: IdEdge) -> bool {
        let Some(edges) = self.0.get(edge.source) else {
            return false;
        };
        edges.contains(&edge)
    }

    pub(super) fn get_edges(&self, node: NodeId) -> Option<&Vec<IdEdge>> {
        self.0.get(node)
    }

    pub(super) fn iter_edges(&self, node: NodeId) -> impl Iterator<Item = &IdEdge> {
        self.0.get(node).into_iter().flat_map(|v| v.iter())
    }

    pub(super) fn get_edges_mut(&mut self, node: NodeId) -> Option<&mut Vec<IdEdge>> {
        self.0.get_mut(node)
    }

    pub(super) fn iter_all_edges(&self) -> impl Iterator<Item = IdEdge> + use<'_> {
        self.0.entries().flat_map(|(_, e)| e.iter().cloned())
    }

    pub(super) fn iter_children(&self, node: NodeId) -> Option<impl Iterator<Item = NodeId> + use<'_>> {
        self.0.get(node).map(|v| v.iter().map(|e| e.target))
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = NodeId> + use<'_> {
        self.0.entries().map(|(n, _)| n)
    }

    pub(super) fn iter_nodes_where(
        &self,
        node: NodeId,
        filter: fn(&IdEdge) -> bool,
    ) -> Option<impl Iterator<Item = NodeId> + use<'_>> {
        self.0
            .get(node)
            .map(move |v| v.iter().filter(move |e| filter(e)).map(|e| e.target))
    }

    pub(super) fn remove_edge(&mut self, edge: IdEdge) {
        self.0
            .get_mut(edge.source)
            .expect("Attempted to remove edge which isn't present.")
            .retain(|e| *e != edge);
    }
}
