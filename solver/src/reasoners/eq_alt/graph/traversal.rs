use std::fmt::Debug;
use std::hash::Hash;

use itertools::Itertools;

use crate::collections::{
    ref_store::{IterableRefMap, RefMap},
    set::{IterableRefSet, RefSet},
};

use super::{IdEdge, NodeId};

pub trait NodeTag: Debug + Eq + Copy + Into<bool> + From<bool> + Hash {}
impl<T: Debug + Eq + Copy + Into<bool> + From<bool> + Hash> NodeTag for T {}

pub trait Fold<T: NodeTag> {
    fn init(&self) -> T;
    /// A function which takes an element of extra stack data and an edge
    /// and returns the new element to add to the stack
    /// None indicates the edge shouldn't be visited
    fn fold(&self, tag: &T, edge: &IdEdge) -> Option<T>;
}

pub trait Graph {
    fn map_source(&self, node: NodeId) -> NodeId;
    fn edges(&self, node: NodeId) -> impl Iterator<Item = IdEdge>;
}

/// Struct allowing for a refined depth first traversal of a Directed Graph in the form of an AdjacencyList.
/// Notably implements the iterator trait
///
/// Performs an operation similar to fold using the stack:
/// Each node can have a annotation of type S
/// The annotation for a new node is calculated from the annotation of the current node and the edge linking the current node to the new node using fold
/// If fold returns None, the edge will not be visited
///
/// This allows to continue traversal while 0 or 1 NEQ edges have been taken, and stop on the second
#[derive(Clone)]
pub struct GraphTraversal<T, F, G>
where
    T: NodeTag,
    F: Fold<T>,
    G: Graph,
{
    /// The graph we're traversing
    graph: G,
    /// Initial element and fold function for node tags
    fold: F,
    /// The set of visited nodes
    visited: IterableRefSet<TaggedNode<T>>,
    // TODO: For best explanations, VecDeque queue should be used with pop_front
    // However, for propagation, Vec is much more performant
    // We should add a generic collection param
    /// The stack of tagged nodes to visit
    stack: Vec<TaggedNode<T>>,
    /// Pass true in order to record paths (if you want to call get_path)
    mem_path: bool,
    /// Records parents of nodes if mem_path is true
    parents: IterableRefMap<TaggedNode<T>, (IdEdge, T)>,
}

impl<T, F, G> GraphTraversal<T, F, G>
where
    T: NodeTag,
    F: Fold<T>,
    G: Graph,
{
    pub fn new(graph: G, fold: F, source: NodeId, mem_path: bool) -> Self {
        GraphTraversal {
            stack: vec![TaggedNode(source, fold.init())],
            graph,
            fold,
            visited: Default::default(),
            mem_path,
            parents: Default::default(),
        }
    }

    /// Get the the path from source to node (in reverse order)
    pub fn get_path(&self, tagged_node: TaggedNode<T>) -> Vec<IdEdge> {
        assert!(self.mem_path, "Set mem_path to true if you want to get path later.");
        let TaggedNode(mut node, mut s) = tagged_node;
        let mut res = Vec::new();
        while let Some((e, new_s)) = self.parents.get(TaggedNode(node, s)) {
            s = *new_s;
            node = e.source;
            res.push(*e);
        }
        res
    }

    pub fn get_reachable(&mut self) -> &IterableRefSet<TaggedNode<T>> {
        while self.next().is_some() {}
        &self.visited
    }
}

impl<T, F, G> Iterator for GraphTraversal<T, F, G>
where
    T: NodeTag,
    F: Fold<T>,
    G: Graph,
{
    type Item = TaggedNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        // Pop a node from the stack
        let mut node = self.stack.pop()?;
        while self.visited.contains(node) {
            node = self.stack.pop()?;
        }

        // Mark as visited
        self.visited.insert(node);

        // Push adjacent edges onto stack according to fold func
        let new_edges = self.graph.edges(node.0).filter_map(|e| {
            // If self.fold returns None, filter edge
            if let Some(s) = self.fold.fold(&node.1, &e) {
                // If edge target visited, filter edge
                let new = TaggedNode(e.target, s);
                if !self.visited.contains(new) {
                    if self.mem_path {
                        self.parents.insert(new, (e, node.1));
                    }
                    Some(new)
                } else {
                    None
                }
            } else {
                None
            }
        });

        self.stack.extend(new_edges);
        Some(node)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TaggedNode<T: NodeTag>(pub NodeId, pub T);

// T gets first bit, N is shifted by one
impl<T: NodeTag> From<usize> for TaggedNode<T> {
    fn from(value: usize) -> Self {
        Self((value >> 1).into(), ((value & 1) != 0).into())
    }
}

impl<T: NodeTag> From<TaggedNode<T>> for usize {
    fn from(value: TaggedNode<T>) -> Self {
        let shift = 1;
        (value.1.into() as usize) | usize::from(value.0) << shift
    }
}
