use hashbrown::HashSet;

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
pub(super) struct Dft<'a, N: AdjNode, E: AdjEdge<N>, S: Copy> {
    /// A directed graph in the form of an adjacency list
    adj_list: &'a AdjacencyList<N, E>,
    /// The set of visited nodes
    visited: HashSet<N>,
    /// The stack of nodes to visit + extra data
    stack: Vec<(N, S)>,
    /// A function which takes an element of extra stack data and an edge
    /// and returns the new element to add to the stack
    /// None indicates the edge shouldn't be visited
    fold: fn(S, &E) -> Option<S>,
}

impl<'a, N: AdjNode, E: AdjEdge<N>, S: Copy> Dft<'a, N, E, S> {
    pub(super) fn new(adj_list: &'a AdjacencyList<N, E>, node: N, init: S, fold: fn(S, &E) -> Option<S>) -> Self {
        Dft {
            adj_list,
            visited: HashSet::new(),
            stack: vec![(node, init)],
            fold,
        }
    }
}

impl<'a, N: AdjNode, E: AdjEdge<N>> Dft<'a, N, E, ()> {
    /// New DFT which doesn't make use of the stack data
    pub(super) fn new_basic(adj_list: &'a AdjacencyList<N, E>, node: N) -> Self {
        Dft {
            adj_list,
            visited: HashSet::new(),
            stack: vec![(node, ())],
            fold: |_, _| Some(()),
        }
    }
}

impl<'a, N: AdjNode, E: AdjEdge<N>, S: Copy> Iterator for Dft<'a, N, E, S> {
    type Item = (N, S);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node, d)) = self.stack.pop() {
            if !self.visited.contains(&node) {
                self.visited.insert(node);

                // Push on to stack edges where mut_stack returns Some
                self.stack.extend(
                    self.adj_list
                        .get_edges(node)
                        .unwrap()
                        .iter()
                        .filter_map(|e| Some((e.target(), (self.fold)(d, e)?))),
                );

                return Some((node, d));
            }
        }
        None
    }
}
