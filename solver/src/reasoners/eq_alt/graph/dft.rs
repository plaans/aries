use hashbrown::{HashMap, HashSet};

use crate::reasoners::eq_alt::graph::{AdjEdge, AdjNode, AdjacencyList};

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
pub struct Dft<'a, N, E, S, F>
where
    N: AdjNode,
    E: AdjEdge<N>,
    F: Fn(&S, &E) -> Option<S>,
{
    /// A directed graph in the form of an adjacency list
    adj_list: &'a AdjacencyList<N, E>,
    /// The set of visited nodes
    visited: HashSet<N>,
    /// The stack of nodes to visit + extra data
    stack: Vec<(N, S)>,
    /// A function which takes an element of extra stack data and an edge
    /// and returns the new element to add to the stack
    /// None indicates the edge shouldn't be visited
    fold: F,
    mem_path: bool,
    parents: HashMap<N, E>,
}

impl<'a, N, E, S, F> Dft<'a, N, E, S, F>
where
    N: AdjNode,
    E: AdjEdge<N>,
    F: Fn(&S, &E) -> Option<S>,
{
    pub(super) fn new(adj_list: &'a AdjacencyList<N, E>, source: N, init: S, fold: F, mem_path: bool) -> Self {
        Dft {
            adj_list,
            visited: HashSet::new(),
            stack: vec![(source, init)],
            fold,
            mem_path,
            parents: Default::default(),
        }
    }

    /// Get the the path from source to node (in reverse order)
    pub fn get_path(&self, mut node: N) -> Vec<E> {
        assert!(self.mem_path, "Set mem_path to true if you want to get path later.");
        let mut res = Vec::new();
        while let Some(e) = self.parents.get(&node) {
            node = e.source();
            res.push(*e);
            // if node == self.source {
            //     break;
            // }
        }
        res
    }
}

impl<'a, N, E, S, F> Iterator for Dft<'a, N, E, S, F>
where
    N: AdjNode,
    E: AdjEdge<N>,
    F: Fn(&S, &E) -> Option<S>,
{
    type Item = (N, S);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node, d)) = self.stack.pop() {
            if !self.visited.contains(&node) {
                self.visited.insert(node);

                // Push adjacent edges onto stack according to fold func
                self.stack
                    .extend(self.adj_list.get_edges(node).unwrap().iter().filter_map(|e| {
                        // If self.fold returns None, filter edge, otherwise stack e.target and self.fold result
                        if let Some(s) = (self.fold)(&d, e) {
                            // Set the edge's target's parent to the current node
                            if self.mem_path && !self.visited.contains(&e.target()) {
                                // debug_assert!(!self.parents.contains_key(&e.target()));
                                self.parents.insert(e.target(), *e);
                            }
                            Some((e.target(), s))
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
