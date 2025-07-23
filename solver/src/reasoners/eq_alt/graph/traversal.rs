use std::fmt::Debug;
use std::hash::Hash;

use crate::{
    collections::{ref_store::RefMap, set::RefSet},
    reasoners::eq_alt::{
        graph::{AdjNode, EqAdjList},
        node::Node,
        relation::EqRelation,
    },
};

use super::Edge;

pub trait NodeTag: Debug + Eq + Copy + Into<bool> + From<bool> {}
impl<T: Debug + Eq + Copy + Into<bool> + From<bool>> NodeTag for T {}
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
pub struct GraphTraversal<'a, N, T, F>
where
    N: AdjNode,
    T: NodeTag,
    F: Fn(&T, &Edge<N>) -> Option<T>,
{
    /// A directed graph in the form of an adjacency list
    adj_list: &'a EqAdjList<N>,
    /// The set of visited nodes
    visited: RefSet<TaggedNode<N, T>>,
    // TODO: For best explanations, VecDeque queue should be used with pop_front
    // However, for propagation, Vec is much more performant
    // We should add a generic collection param
    /// The stack of tagged nodes to visit
    stack: Vec<TaggedNode<N, T>>,
    /// A function which takes an element of extra stack data and an edge
    /// and returns the new element to add to the stack
    /// None indicates the edge shouldn't be visited
    fold: F,
    /// Pass true in order to record paths (if you want to call get_path)
    mem_path: bool,
    /// Records parents of nodes if mem_path is true
    parents: RefMap<TaggedNode<N, T>, (Edge<N>, T)>,
}

impl<'a, N, T, F> GraphTraversal<'a, N, T, F>
where
    N: AdjNode + Into<usize> + From<usize>,
    T: Eq + Hash + Copy + Debug + Into<bool> + From<bool>,
    F: Fn(&T, &Edge<N>) -> Option<T>,
{
    pub(super) fn new(adj_list: &'a EqAdjList<N>, source: N, init: T, fold: F, mem_path: bool) -> Self {
        // TODO: For performance, maybe create queue with capacity
        GraphTraversal {
            adj_list,
            visited: RefSet::with_capacity(adj_list.capacity()),
            stack: [TaggedNode(source, init)].into(),
            fold,
            mem_path,
            parents: Default::default(),
        }
    }

    /// Get the the path from source to node (in reverse order)
    pub fn get_path(&self, TaggedNode(mut node, mut s): TaggedNode<N, T>) -> Vec<Edge<N>> {
        assert!(self.mem_path, "Set mem_path to true if you want to get path later.");
        let mut res = Vec::new();
        while let Some((e, new_s)) = self.parents.get(TaggedNode(node, s)) {
            s = *new_s;
            node = e.source;
            res.push(*e);
            // if node == self.source {
            //     break;
            // }
        }
        res
    }

    pub fn get_reachable(&mut self) -> &RefSet<TaggedNode<N, T>> {
        while self.next().is_some() {}
        &self.visited
    }
}

impl<'a, N, T, F> Iterator for GraphTraversal<'a, N, T, F>
where
    N: AdjNode,
    T: NodeTag,
    F: Fn(&T, &Edge<N>) -> Option<T>,
{
    type Item = TaggedNode<N, T>;

    fn next(&mut self) -> Option<Self::Item> {
        // Pop a node from the stack. We know it hasn't been visited since we check before pushing
        if let Some(TaggedNode(node, d)) = self.stack.pop() {
            // Mark as visited
            self.visited.insert(TaggedNode(node, d));

            // Push adjacent edges onto stack according to fold func
            self.stack
                .extend(self.adj_list.get_edges(node).unwrap().iter().filter_map(|e| {
                    // If self.fold returns None, filter edge
                    if let Some(s) = (self.fold)(&d, e) {
                        // If edge target visited, filter edge
                        if !self.visited.contains(TaggedNode(e.target, s)) {
                            if self.mem_path {
                                self.parents.insert(TaggedNode(e.target, s), (*e, d));
                            }
                            Some(TaggedNode(e.target, s))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }));

            Some(TaggedNode(node, d))
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TaggedNode<N, T>(pub N, pub T)
where
    N: AdjNode,
    T: NodeTag;

// T gets first bit, N is shifted by one
impl<N, T> From<usize> for TaggedNode<N, T>
where
    N: AdjNode,
    T: NodeTag,
{
    fn from(value: usize) -> Self {
        Self((value >> 1).into(), ((value & 1) != 0).into())
    }
}

impl<N, T> From<TaggedNode<N, T>> for usize
where
    N: AdjNode,
    T: NodeTag,
{
    fn from(value: TaggedNode<N, T>) -> Self {
        let shift = 1;
        (value.1.into() as usize) | value.0.into() << shift
    }
}

// Into and From ints for types this is intended to be used with
//
// Node type gets bit 1
// Node var gets shifted by 1
// Node val sign gets bit 2
// Node val abs gets shifted by 1
impl From<usize> for Node {
    fn from(value: usize) -> Self {
        if value & 1 == 0 {
            Node::Var((value >> 1).into())
        } else if value & 0b10 == 0 {
            Node::Val((value >> 2) as i32)
        } else {
            Node::Val(-((value >> 2) as i32))
        }
    }
}

impl From<Node> for usize {
    fn from(value: Node) -> Self {
        match value {
            Node::Var(v) => usize::from(v) << 1,
            Node::Val(v) => {
                if v >= 0 {
                    (v as usize) << 2 | 1
                } else {
                    (-v as usize) << 2 | 0b11
                }
            }
        }
    }
}

impl From<bool> for EqRelation {
    fn from(value: bool) -> Self {
        if value {
            EqRelation::Eq
        } else {
            EqRelation::Neq
        }
    }
}

impl From<EqRelation> for bool {
    fn from(value: EqRelation) -> Self {
        match value {
            EqRelation::Eq => true,
            EqRelation::Neq => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::VarRef;

    #[test]
    fn test_conversion() {
        let cases = [
            TaggedNode(Node::Var(VarRef::from_u32(1)), EqRelation::Eq),
            TaggedNode(Node::Val(-10), EqRelation::Eq),
            TaggedNode(Node::Val(-10), EqRelation::Neq),
        ];
        for case in cases {
            let u: usize = case.into();
            assert_eq!(case, u.into());
        }
    }
}
