#![allow(unused)]

use std::{
    fmt::{Debug, Display, Formatter},
    hash::Hash,
};

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;

use crate::{
    collections::{
        ref_store::{IterableRefMap, RefMap},
        set::{IterableRefSet, RefSet},
    },
    reasoners::eq_alt::relation::EqRelation,
};

use super::{
    traversal::{GraphTraversal, TaggedNode},
    Edge,
};

pub trait AdjNode: Eq + Hash + Copy + Debug + Into<usize> + From<usize> {}

impl<T: Eq + Hash + Copy + Debug + Into<usize> + From<usize>> AdjNode for T {}

#[derive(Default, Clone)]
pub(super) struct EqAdjList<N: AdjNode>(IterableRefMap<N, Vec<Edge<N>>>);

impl<N: AdjNode> Debug for EqAdjList<N> {
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

impl<N: AdjNode> EqAdjList<N> {
    pub(super) fn new() -> Self {
        Self(Default::default())
    }

    /// Insert a node if not present, returns None if node was inserted, else Some(edges)
    pub(super) fn insert_node(&mut self, node: N) -> Option<Vec<Edge<N>>> {
        if !self.0.contains(node) {
            self.0.insert(node, Default::default());
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
                edges.push(edge);
                true
            },
        )
    }

    pub fn contains_edge(&self, edge: Edge<N>) -> bool {
        let Some(edges) = self.0.get(edge.source) else {
            return false;
        };
        edges.contains(&edge)
    }

    pub(super) fn get_edges(&self, node: N) -> Option<&Vec<Edge<N>>> {
        self.0.get(node)
    }

    pub(super) fn get_edges_mut(&mut self, node: N) -> Option<&mut Vec<Edge<N>>> {
        self.0.get_mut(node)
    }

    pub(super) fn iter_all_edges(&self) -> impl Iterator<Item = Edge<N>> + use<'_, N> {
        self.0.entries().flat_map(|(_, e)| e.iter().cloned())
    }

    pub(super) fn iter_children(&self, node: N) -> Option<impl Iterator<Item = N> + use<'_, N>> {
        self.0.get(node).map(|v| v.iter().map(|e| e.target))
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = N> + use<'_, N> {
        self.0.entries().map(|(n, _)| n)
    }

    pub(super) fn iter_nodes_where(
        &self,
        node: N,
        filter: fn(&Edge<N>) -> bool,
    ) -> Option<impl Iterator<Item = N> + use<'_, N>> {
        self.0
            .get(node)
            .map(move |v| v.iter().filter(move |e: &&Edge<N>| filter(*e)).map(|e| e.target))
    }

    pub(super) fn remove_edge(&mut self, node: N, edge: Edge<N>) {
        self.0
            .get_mut(node)
            .expect("Attempted to remove edge which isn't present.")
            .retain(|e| *e != edge);
    }

    pub fn eq_traversal<F>(
        &self,
        source: N,
        filter: F,
    ) -> GraphTraversal<'_, N, bool, impl Fn(&bool, &Edge<N>) -> Option<bool>>
    where
        F: Fn(&Edge<N>) -> bool,
    {
        GraphTraversal::new(
            self,
            source,
            false,
            move |_, e| (e.relation == EqRelation::Eq && filter(e)).then_some(false),
            false,
        )
    }

    /// IMPORTANT: relation passed to filter closure is relation that node will be reached with
    pub fn eq_or_neq_traversal<F>(
        &self,
        source: N,
        filter: F,
    ) -> GraphTraversal<'_, N, EqRelation, impl Fn(&EqRelation, &Edge<N>) -> Option<EqRelation>>
    where
        F: Fn(&Edge<N>, &EqRelation) -> bool,
    {
        GraphTraversal::new(
            self,
            source,
            EqRelation::Eq,
            move |r, e| (*r + e.relation).filter(|new_r| filter(e, new_r)),
            false,
        )
    }

    pub fn eq_path_traversal<F>(
        &self,
        node: N,
        filter: F,
    ) -> GraphTraversal<'_, N, bool, impl Fn(&bool, &Edge<N>) -> Option<bool>>
    where
        F: Fn(&Edge<N>) -> bool,
    {
        GraphTraversal::new(
            self,
            node,
            false,
            move |_, e| {
                if filter(e) {
                    match e.relation {
                        EqRelation::Eq => Some(false),
                        EqRelation::Neq => None,
                    }
                } else {
                    None
                }
            },
            true,
        )
    }

    /// Util for traversal while 0 or 1 neqs
    pub fn eq_or_neq_path_traversal<F>(
        &self,
        node: N,
        filter: F,
    ) -> GraphTraversal<N, EqRelation, impl Fn(&EqRelation, &Edge<N>) -> Option<EqRelation>>
    where
        F: Fn(&Edge<N>) -> bool,
    {
        GraphTraversal::new(
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

    pub fn eq_reachable_from(&self, source: N) -> RefSet<TaggedNode<N, bool>> {
        self.eq_traversal(source, |_| true).get_reachable().clone()
    }

    pub fn eq_or_neq_reachable_from(&self, source: N) -> RefSet<TaggedNode<N, EqRelation>> {
        self.eq_or_neq_traversal(source, |_, _| true).get_reachable().clone()
    }

    pub(crate) fn n_nodes(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn capacity(&self) -> usize {
        self.0.capacity()
    }

    #[allow(deprecated)]
    pub fn print_stats(&self) {
        println!("N nodes: {}", self.n_nodes());
        println!("Capacity: {}", self.capacity());
        println!("N edges: {}", self.iter_all_edges().count());
        let mut reached: HashSet<(N, EqRelation)> = HashSet::new();
        let mut group_sizes = vec![];
        for (n, r) in self
            .iter_nodes()
            .cartesian_product(vec![EqRelation::Eq, EqRelation::Neq])
        {
            if reached.contains(&(n, r)) {
                continue;
            }
            let mut group_size = 0_usize;
            if r == EqRelation::Eq {
                self.eq_or_neq_reachable_from(n).iter().for_each(|TaggedNode(np, rp)| {
                    reached.insert((np, rp));
                    group_size += 1;
                });
            } else {
                self.eq_reachable_from(n).iter().for_each(|TaggedNode(np, _)| {
                    reached.insert((np, EqRelation::Neq));
                    group_size += 1;
                });
            }
            group_sizes.push(group_size);
        }
        println!(
            "Average group size: {}",
            group_sizes.iter().sum::<usize>() / group_sizes.len()
        );
        println!("Maximum group size: {:?}", group_sizes.iter().max());
    }
}
