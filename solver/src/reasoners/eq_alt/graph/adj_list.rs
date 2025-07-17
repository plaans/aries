#![allow(unused)]

use std::{
    fmt::{Debug, Display, Formatter},
    hash::Hash,
};

use hashbrown::{HashMap, HashSet};

use crate::reasoners::eq_alt::relation::EqRelation;

use super::{bft::Bft, Edge};

pub trait AdjNode: Eq + Hash + Copy + Debug {}

impl<T: Eq + Hash + Copy + Debug> AdjNode for T {}

#[derive(Default, Clone)]
pub(super) struct EqAdjList<N: AdjNode>(HashMap<N, HashSet<Edge<N>>>);

impl<N: AdjNode> Debug for EqAdjList<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f)?;
        for (node, edges) in &self.0 {
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

impl<N: AdjNode> EqAdjList<N> {
    pub(super) fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert a node if not present, returns None if node was inserted, else Some(edges)
    pub(super) fn insert_node(&mut self, node: N) -> Option<Vec<Edge<N>>> {
        if !self.0.contains_key(&node) {
            self.0.insert(node, HashSet::new());
        }
        None
    }

    /// Insert an edge and possibly a node
    /// First return val is if source node was inserted, second is if target val was inserted, third is if edge was inserted
    pub(super) fn insert_edge(&mut self, node: N, edge: Edge<N>) -> (bool, bool, bool) {
        let node_added = self.insert_node(node).is_none();
        let target_added = self.insert_node(edge.target).is_none();
        let edges = self.get_edges_mut(node).unwrap();
        (
            node_added,
            target_added,
            if edges.contains(&edge) {
                false
            } else {
                edges.insert(edge);
                true
            },
        )
    }

    pub fn contains_edge(&self, edge: Edge<N>) -> bool {
        let Some(edges) = self.0.get(&edge.source) else {
            return false;
        };
        edges.contains(&edge)
    }

    pub(super) fn get_edges(&self, node: N) -> Option<&HashSet<Edge<N>>> {
        self.0.get(&node)
    }

    pub(super) fn get_edges_mut(&mut self, node: N) -> Option<&mut HashSet<Edge<N>>> {
        self.0.get_mut(&node)
    }

    pub(super) fn iter_all_edges(&self) -> impl Iterator<Item = Edge<N>> + use<'_, N> {
        self.0.iter().flat_map(|(_, e)| e.iter().cloned())
    }

    pub(super) fn iter_children(&self, node: N) -> Option<impl Iterator<Item = N> + use<'_, N>> {
        self.0.get(&node).map(|v| v.iter().map(|e| e.target))
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = N> + use<'_, N> {
        self.0.iter().map(|(n, _)| *n)
    }

    pub(super) fn iter_nodes_where(
        &self,
        node: N,
        filter: fn(&Edge<N>) -> bool,
    ) -> Option<impl Iterator<Item = N> + use<'_, N>> {
        self.0
            .get(&node)
            .map(move |v| v.iter().filter(move |e: &&Edge<N>| filter(*e)).map(|e| e.target))
    }

    pub(super) fn remove_edge(&mut self, node: N, edge: Edge<N>) -> bool {
        self.0
            .get_mut(&node)
            .expect("Attempted to remove edge which isn't present.")
            .remove(&edge)
    }

    pub(super) fn allocated(&self) -> usize {
        self.0.allocation_size() + self.0.iter().fold(0, |v, e| e.1.allocation_size())
    }

    pub fn eq_bft(&self, source: N) -> impl Iterator<Item = N> + use<'_, N> + Clone {
        Bft::new(
            self,
            source,
            (),
            |_, e| match e.relation {
                EqRelation::Eq => Some(()),
                EqRelation::Neq => None,
            },
            false,
        )
        .map(|(e, _)| e)
    }

    pub fn eq_or_neq_bft(&self, source: N) -> impl Iterator<Item = (N, EqRelation)> + use<'_, N> + Clone {
        Bft::new(self, source, EqRelation::Eq, move |r, e| *r + e.relation, false)
    }

    pub fn eq_path_bft<'a>(
        &'a self,
        node: N,
        filter: impl Fn(&Edge<N>) -> bool + 'a,
    ) -> Bft<'a, N, (), impl Fn(&(), &Edge<N>) -> Option<()>> {
        Bft::new(
            self,
            node,
            (),
            move |_, e| {
                if filter(e) {
                    match e.relation {
                        EqRelation::Eq => Some(()),
                        EqRelation::Neq => None,
                    }
                } else {
                    None
                }
            },
            true,
        )
    }

    /// Util for bft while 0 or 1 neqs
    pub fn eq_or_neq_path_bft<'a>(
        &'a self,
        node: N,
        filter: impl Fn(&Edge<N>) -> bool + 'a,
    ) -> Bft<'a, N, EqRelation, impl Fn(&EqRelation, &Edge<N>) -> Option<EqRelation>> {
        Bft::new(
            self,
            node,
            EqRelation::Eq,
            move |r, e| {
                if filter(e) {
                    *r + e.relation
                } else {
                    None
                }
            },
            true,
        )
    }

    // pub fn reachable_from(&self, node: N) -> HashSet<N> {
    // let res = HashSet::new();
    // }
}
