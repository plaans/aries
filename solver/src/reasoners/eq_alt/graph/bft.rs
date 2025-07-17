use hashbrown::{HashMap, HashSet};
use std::{collections::VecDeque, hash::Hash};

use crate::reasoners::eq_alt::graph::{AdjNode, EqAdjList};

use super::Edge;

/// Struct allowing for a refined depth first traversal of a Directed Graph in the form of an AdjacencyList.
/// Notably implements the iterator trait
///
/// Performs an operation similar to fold using the stack:
/// Each node can have a annotation of type S
/// The annotation for a new node is calculated from the annotation of the current node and the edge linking the current node to the new node using fold
/// If fold returns None, the edge will not be visited
///
/// This allows to continue traversal while 0 or 1 NEQ edges have been taken, and stop on the second
#[derive(Clone, Debug)]
pub struct Bft<'a, N, S, F>
where
    N: AdjNode,
    S: Eq + Hash + Copy,
    F: Fn(&S, &Edge<N>) -> Option<S>,
{
    /// A directed graph in the form of an adjacency list
    adj_list: &'a EqAdjList<N>,
    /// The set of visited nodes
    visited: HashSet<(N, S)>,
    /// The stack of nodes to visit + extra data
    queue: VecDeque<(N, S)>,
    /// A function which takes an element of extra stack data and an edge
    /// and returns the new element to add to the stack
    /// None indicates the edge shouldn't be visited
    fold: F,
    /// Pass true in order to record paths (if you want to call get_path)
    mem_path: bool,
    /// Records parents of nodes if mem_path is true
    parents: HashMap<(N, S), (Edge<N>, S)>,
}

impl<'a, N, S, F> Bft<'a, N, S, F>
where
    N: AdjNode,
    S: Eq + Hash + Copy,
    F: Fn(&S, &Edge<N>) -> Option<S>,
{
    pub(super) fn new(adj_list: &'a EqAdjList<N>, source: N, init: S, fold: F, mem_path: bool) -> Self {
        Bft {
            adj_list,
            visited: HashSet::new(),
            queue: [(source, init)].into(),
            fold,
            mem_path,
            parents: Default::default(),
        }
    }

    /// Get the the path from source to node (in reverse order)
    pub fn get_path(&self, mut node: N, mut s: S) -> Vec<Edge<N>> {
        assert!(self.mem_path, "Set mem_path to true if you want to get path later.");
        let mut res = Vec::new();
        while let Some((e, new_s)) = self.parents.get(&(node, s)) {
            s = *new_s;
            node = e.source;
            res.push(*e);
            // if node == self.source {
            //     break;
            // }
        }
        res
    }
}

impl<'a, N, S, F> Iterator for Bft<'a, N, S, F>
where
    N: AdjNode,
    S: Eq + Hash + Copy,
    F: Fn(&S, &Edge<N>) -> Option<S>,
{
    type Item = (N, S);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node, d)) = self.queue.pop_front() {
            if !self.visited.contains(&(node, d)) {
                self.visited.insert((node, d));

                // Push adjacent edges onto stack according to fold func
                self.queue
                    .extend(self.adj_list.get_edges(node).unwrap().iter().filter_map(|e| {
                        // If self.fold returns None, filter edge, otherwise stack e.target and self.fold result
                        if let Some(s) = (self.fold)(&d, e) {
                            // Set the edge's target's parent to the current node
                            if self.mem_path && !self.visited.contains(&(e.target, s)) {
                                // debug_assert!(!self.parents.contains_key(&e.target()));
                                self.parents.insert((e.target, s), (*e, d));
                            }
                            Some((e.target, s))
                        } else {
                            None
                        }
                    }));

                return Some((node, d));
            }
        }
        None
    }
}
