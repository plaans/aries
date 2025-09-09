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

    fn traverse<'a>(self, source: N, scratch: &'a mut Scratch) -> GraphTraversal<'a, N, E, Self>
    where
        Self: Sized,
    {
        GraphTraversal::new(self, source, scratch)
    }

    fn reachable<'a>(self, source: N, scratch: &'a mut Scratch) -> Visited<'a, N>
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

#[derive(Default)]
pub struct Scratch {
    stack: Vec<usize>,
    visited: IterableRefSet<usize>,
}

struct MutStack<'a, N: Into<usize> + From<usize>>(&'a mut Vec<usize>, std::marker::PhantomData<N>);

impl<'a, N: Into<usize> + From<usize>> MutStack<'a, N> {
    fn new(s: &'a mut Vec<usize>) -> Self {
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

pub struct MutVisited<'a, N: Into<usize> + From<usize>>(&'a mut IterableRefSet<usize>, std::marker::PhantomData<N>);
pub struct Visited<'a, N: Into<usize> + From<usize>>(&'a IterableRefSet<usize>, std::marker::PhantomData<N>);

impl<'a, N: Into<usize> + From<usize>> MutVisited<'a, N> {
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
impl<'a, N: Into<usize> + From<usize>> Visited<'a, N> {
    fn new(v: &'a IterableRefSet<usize>) -> Self {
        Self(v, std::marker::PhantomData {})
    }

    pub fn contains(&self, n: N) -> bool {
        self.0.contains(n.into())
    }
}

impl Scratch {
    fn stack<'a, N: Into<usize> + From<usize>>(&'a mut self) -> MutStack<'a, N> {
        MutStack::new(&mut self.stack)
    }

    fn visited_mut<'a, N: Into<usize> + From<usize>>(&'a mut self) -> MutVisited<'a, N> {
        MutVisited::new(&mut self.visited)
    }

    fn visited<'a, N: Into<usize> + From<usize>>(&'a self) -> Visited<'a, N> {
        Visited::new(&self.visited)
    }

    fn clear(&mut self) {
        self.stack.clear();
        self.visited.clear();
    }
}

pub struct GraphTraversal<'a, N: Ref, E: Edge<N>, G: Graph<N, E>> {
    graph: G,
    scratch: &'a mut Scratch,
    parents: Option<&'a mut PathStore<N, E>>,
}

impl<'a, N: Ref, E: Edge<N>, G: Graph<N, E>> GraphTraversal<'a, N, E, G> {
    pub fn new(graph: G, source: N, scratch: &'a mut Scratch) -> Self {
        scratch.clear();
        scratch.stack().push(source);
        GraphTraversal {
            graph,
            scratch,
            parents: None,
        }
    }

    pub fn mem_path(mut self, path_store: &'a mut PathStore<N, E>) -> Self {
        debug_assert!(self.parents.is_none());
        debug_assert!(self.scratch.visited.is_empty());
        self.parents = Some(path_store);
        self
    }

    pub fn visited(&self) -> Visited<'_, N> {
        self.scratch.visited()
    }
}

impl<N: Ref, E: Edge<N>, G: Graph<N, E>> Iterator for GraphTraversal<'_, N, E, G> {
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        let mut node = self.scratch.stack().pop()?;
        while self.scratch.visited_mut().contains(node) {
            node = self.scratch.stack().pop()?;
        }

        self.scratch.visited_mut().insert(node);

        let mut stack = MutStack::new(&mut self.scratch.stack);
        let visited = Visited::new(&self.scratch.visited);

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
