use std::collections::VecDeque;

use crate::collections::{
    ref_store::{IterableRefMap, Ref},
    set::IterableRefSet,
};

pub trait Node: Ref {}
impl<T: Ref> Node for T {}

/// A trait representing a generic directed edge with a source and target.
pub trait Edge<N>: Clone {
    fn target(&self) -> N;
    fn source(&self) -> N;
}

/// A trait representing a generic directed Graph.
pub trait Graph<N: Node, E: Edge<N>> {
    /// Get outgoing edges from the node.
    fn outgoing(&self, node: N) -> impl Iterator<Item = E>;

    /// Traverse the graph (depth first) from a given source. This method return a GraphTraversal object which implements Iterator.
    ///
    /// Scratch contains the large data structures used by the graph traversal algorithm. Useful to reuse memory.
    /// `&mut default::default()` can used if performance is not critical.
    fn traverse_dfs<'a>(
        self,
        source: N,
        scratch: &'a mut Scratch<Vec<usize>>,
    ) -> GraphTraversal<'a, N, E, Self, Vec<usize>>
    where
        Self: Sized,
    {
        GraphTraversal::new(self, source, scratch)
    }

    /// Traverse the graph (breadth first) from a given source. This method return a GraphTraversal object which implements Iterator.
    ///
    /// Scratch contains the large data structures used by the graph traversal algorithm. Useful to reuse memory.
    /// `&mut default::default()` can used if performance is not critical.
    fn traverse_bfs<'a>(
        self,
        source: N,
        scratch: &'a mut Scratch<VecDeque<usize>>,
    ) -> GraphTraversal<'a, N, E, Self, VecDeque<usize>>
    where
        Self: Sized,
    {
        GraphTraversal::new(self, source, scratch)
    }

    /// Get the set of nodes which can be reached from the source.
    ///
    /// See traverse for details about scratch.
    fn reachable<'a>(self, source: N, scratch: &'a mut Scratch<Vec<usize>>) -> Visited<'a, N>
    where
        Self: Sized + 'a,
        N: 'a,
        E: 'a,
    {
        let mut t = GraphTraversal::new(self, source, scratch);
        for _ in t.by_ref() {}
        scratch.visited()
    }
}

pub trait Frontier<N> {
    fn push(&mut self, value: N);

    fn pop(&mut self) -> Option<N>;

    fn extend(&mut self, values: impl IntoIterator<Item = N>);

    fn clear(&mut self);
}

impl<N> Frontier<N> for Vec<N> {
    fn push(&mut self, value: N) {
        self.push(value);
    }

    fn pop(&mut self) -> Option<N> {
        self.pop()
    }

    fn extend(&mut self, values: impl IntoIterator<Item = N>) {
        Extend::extend(self, values)
    }

    fn clear(&mut self) {
        self.clear()
    }
}

impl<N> Frontier<N> for VecDeque<N> {
    fn push(&mut self, value: N) {
        self.push_back(value);
    }

    fn pop(&mut self) -> Option<N> {
        self.pop_front()
    }

    fn extend(&mut self, values: impl IntoIterator<Item = N>) {
        Extend::extend(self, values);
    }

    fn clear(&mut self) {
        self.clear()
    }
}

/// A data structure that can be passed to GraphTraversal in order to record parents of visited nodes.
/// This allows for path queries after traversal.
///
/// Call record_paths on GraphTraversal with this struct.
pub struct PathStore<N: Node, E: Edge<N>>(IterableRefMap<N, E>);

impl<N: Node, E: Edge<N>> PathStore<N, E> {
    pub fn new() -> Self {
        Self(Default::default())
    }

    pub fn get_path(&self, mut target: N) -> impl Iterator<Item = E> + use<'_, N, E> {
        std::iter::from_fn(move || {
            self.0.get(target).map(|e| {
                target = e.source();
                e.clone()
            })
        })
    }
}

/// Scratch contains the large data structures used by the graph traversal algorithm. Useful to reuse memory.
///
/// In order to avoid having to deal with generics when reusing an instance, we use usize instead of N: Into\<usize> + From\<usize>.
/// We therefore need structs to access these data structures with N.
#[derive(Default)]
pub struct Scratch<F: Frontier<usize>> {
    frontier: F,
    visited: IterableRefSet<usize>,
}

/// Used to access Scratch.stack as if it were `Vec<N>`
struct FrontierMut<'a, N: Into<usize> + From<usize>, F: Frontier<usize>>(&'a mut F, std::marker::PhantomData<N>);

impl<'a, N: Into<usize> + From<usize>, F: Frontier<usize>> FrontierMut<'a, N, F> {
    fn new(s: &'a mut F) -> Self {
        Self(s, std::marker::PhantomData {})
    }

    fn push(&mut self, n: N) {
        self.0.push(n.into())
    }

    fn pop(&mut self) -> Option<N> {
        self.0.pop().map(Into::into)
    }

    fn extend(&mut self, iter: impl IntoIterator<Item = N>) {
        self.0.extend(iter.into_iter().map(Into::into))
    }
}

/// Used to access Scratch.visited as if it were `IterableRefSet<N>`
pub struct VisitedMut<'a, N: Into<usize> + From<usize>>(&'a mut IterableRefSet<usize>, std::marker::PhantomData<N>);

impl<'a, N: Into<usize> + From<usize>> VisitedMut<'a, N> {
    fn new(v: &'a mut IterableRefSet<usize>) -> Self {
        Self(v, std::marker::PhantomData {})
    }

    pub fn contains(&mut self, n: N) -> bool {
        self.0.contains(n.into())
    }

    pub fn insert(&mut self, n: N) {
        self.0.insert(n.into())
    }
}

/// Used to access Scratch.visited as if it were `IterableRefSet<N>`
pub struct Visited<'a, N: Into<usize> + From<usize>>(&'a IterableRefSet<usize>, std::marker::PhantomData<N>);
impl<'a, N: Into<usize> + From<usize>> Visited<'a, N> {
    fn new(v: &'a IterableRefSet<usize>) -> Self {
        Self(v, std::marker::PhantomData {})
    }

    pub fn contains(&self, n: N) -> bool {
        self.0.contains(n.into())
    }
}

impl<F: Frontier<usize>> Scratch<F> {
    fn stack<'a, N: Into<usize> + From<usize>>(&'a mut self) -> FrontierMut<'a, N, F> {
        FrontierMut::new(&mut self.frontier)
    }

    fn visited_mut<'a, N: Into<usize> + From<usize>>(&'a mut self) -> VisitedMut<'a, N> {
        VisitedMut::new(&mut self.visited)
    }

    fn visited<'a, N: Into<usize> + From<usize>>(&'a self) -> Visited<'a, N> {
        Visited::new(&self.visited)
    }

    fn clear(&mut self) {
        self.frontier.clear();
        self.visited.clear();
    }
}

/// Struct for traversing a Graph with DFS or BFS.
/// Implements iterator.
pub struct GraphTraversal<'a, N: Node, E: Edge<N>, G: Graph<N, E>, F: Frontier<usize>> {
    graph: G,
    scratch: &'a mut Scratch<F>,
    parents: Option<&'a mut PathStore<N, E>>,
}

impl<'a, N: Node, E: Edge<N>, G: Graph<N, E>, F: Frontier<usize>> GraphTraversal<'a, N, E, G, F> {
    fn new(graph: G, source: N, scratch: &'a mut Scratch<F>) -> Self {
        scratch.clear();
        scratch.stack().push(source);
        GraphTraversal {
            graph,
            scratch,
            parents: None,
        }
    }

    /// Record paths taken during traversal to PathStore, allowing for path from source to visited node queries.
    pub fn record_paths(mut self, path_store: &'a mut PathStore<N, E>) -> Self {
        // TODO: We should make this safe by introducing a new type for iteration
        debug_assert!(self.parents.is_none());
        debug_assert!(self.scratch.visited.is_empty());
        self.parents = Some(path_store);
        self
    }

    pub fn visited(&self) -> Visited<'_, N> {
        self.scratch.visited()
    }
}

impl<N: Node, E: Edge<N>, G: Graph<N, E>, F: Frontier<usize>> Iterator for GraphTraversal<'_, N, E, G, F> {
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        // Get the next unvisited node
        let mut node = self.scratch.stack().pop()?;
        while self.scratch.visited_mut().contains(node) {
            node = self.scratch.stack().pop()?;
        }

        // Mark as visited
        self.scratch.visited_mut().insert(node);

        let mut stack = FrontierMut::new(&mut self.scratch.frontier);
        let visited = Visited::new(&self.scratch.visited);

        // Get all (unvisited) nodes that can be reached through an outgoing edge
        let new_nodes = self.graph.outgoing(node).filter_map(|e| {
            let target = e.target();
            if !visited.contains(target) {
                if let Some(parents) = self.parents.as_mut() {
                    parents.0.insert(target, e);
                }
                Some(target)
            } else {
                None
            }
        });
        stack.extend(new_nodes);

        Some(node)
    }
}
