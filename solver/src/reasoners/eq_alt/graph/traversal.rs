use crate::collections::{
    ref_store::{IterableRefMap, Ref},
    set::IterableRefSet,
};

pub trait Edge<N>: Clone {
    fn target(&self) -> N;
    fn source(&self) -> N;
}

pub trait Graph<N: Ref, E: Edge<N>> {
    fn outgoing(&self, node: N) -> impl Iterator<Item = E>;

    fn traverse(self, source: N) -> GraphTraversal<'static, N, E, Self>
    where
        Self: Sized,
    {
        GraphTraversal::new(self, source)
    }

    fn reachable(self, source: N) -> IterableRefSet<N>
    where
        Self: Sized,
    {
        let mut t = GraphTraversal::new(self, source);
        for _ in t.by_ref() {}
        t.visited.clone()
    }
}

pub struct PathStore<N: Ref, E: Edge<N>>(IterableRefMap<N, E>);

impl<N: Ref, E: Edge<N>> PathStore<N, E> {
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

pub struct GraphTraversal<'a, N: Ref, E: Edge<N>, G: Graph<N, E>> {
    graph: G,
    stack: Vec<N>,
    visited: IterableRefSet<N>,
    parents: Option<&'a mut PathStore<N, E>>,
}

impl<'a, N: Ref, E: Edge<N>, G: Graph<N, E>> GraphTraversal<'a, N, E, G> {
    pub fn new(graph: G, source: N) -> Self {
        GraphTraversal {
            graph,
            stack: vec![source],
            visited: Default::default(),
            parents: None,
        }
    }

    pub fn mem_path(mut self, path_store: &'a mut PathStore<N, E>) -> Self {
        debug_assert!(self.parents.is_none());
        debug_assert!(self.visited.is_empty());
        self.parents = Some(path_store);
        self
    }

    pub fn visited(&self) -> &IterableRefSet<N> {
        &self.visited
    }
}

impl<N: Ref, E: Edge<N>, G: Graph<N, E>> Iterator for GraphTraversal<'_, N, E, G> {
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        let mut node = self.stack.pop()?;
        while self.visited.contains(node) {
            node = self.stack.pop()?;
        }

        self.visited.insert(node);

        let new_nodes = self.graph.outgoing(node).filter_map(|e| {
            let target = e.target();
            if !self.visited.contains(target) {
                if let Some(parents) = self.parents.as_mut() {
                    parents.0.insert(target, e);
                }
                Some(target)
            } else {
                None
            }
        });
        self.stack.extend(new_nodes);
        Some(node)
    }
}
