use std::{
    fmt::{Debug, Formatter},
    hash::Hash,
};

use hashbrown::HashMap;

pub trait AdjEdge<N>: Eq + Copy + Debug {
    fn target(&self) -> N;
}

pub trait AdjNode: Eq + Hash + Copy + Debug {}

impl<T: Eq + Hash + Copy + Debug> AdjNode for T {}

#[derive(Default, Clone)]
pub(super) struct AdjacencyList<N: AdjNode, E: AdjEdge<N>>(HashMap<N, Vec<E>>);

impl<N: AdjNode, E: AdjEdge<N>> Debug for AdjacencyList<N, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f);
        for (node, edges) in &self.0 {
            writeln!(f, "{:?}:", node)?;
            if edges.is_empty() {
                writeln!(f, "  (no edges)")?;
            } else {
                for edge in edges {
                    writeln!(f, "  -> {:?}", edge.target())?;
                }
            }
        }
        Ok(())
    }
}

impl<N: AdjNode, E: AdjEdge<N>> AdjacencyList<N, E> {
    pub(super) fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert a node if not present, returns None if node was inserted, else Some(edges)
    pub(super) fn insert_node(&mut self, node: N) -> Option<Vec<E>> {
        if !self.0.contains_key(&node) {
            self.0.insert(node, vec![]);
        }
        None
    }

    /// Insert an edge and possibly a node
    /// First return val is if source node was inserted, second is if target val was inserted, third is if edge was inserted
    pub(super) fn insert_edge(&mut self, node: N, edge: E) -> (bool, bool, bool) {
        let node_added = self.insert_node(node).is_none();
        let target_added = self.insert_node(edge.target()).is_none();
        let edges = self.get_edges_mut(node).unwrap();
        (
            node_added,
            target_added,
            if edges.contains(&edge) {
                false
            } else {
                edges.push(edge);
                true
            },
        )
    }

    pub(super) fn get_edges(&self, node: N) -> Option<&Vec<E>> {
        self.0.get(&node)
    }

    pub(super) fn get_edges_mut(&mut self, node: N) -> Option<&mut Vec<E>> {
        self.0.get_mut(&node)
    }

    pub(super) fn iter_nodes(&self, node: N) -> Option<impl Iterator<Item = N> + use<'_, N, E>> {
        self.0.get(&node).map(|v| v.iter().map(|e| e.target()))
    }

    pub(super) fn iter_nodes_where(
        &self,
        node: N,
        filter: fn(&E) -> bool,
    ) -> Option<impl Iterator<Item = N> + use<'_, N, E>> {
        self.0
            .get(&node)
            .map(move |v| v.iter().filter(move |e: &&E| filter(*e)).map(|e| e.target()))
    }
}
