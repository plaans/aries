use crate::core::literals::{LitSet, Watches};
use crate::core::*;
use std::num::NonZeroUsize;
use std::sync::Mutex;

/// An implication in the form of a 2-SAT network.
///
/// It allows answering in polynomial time whether a given literal implies another one (directly or indirectly)
///
/// The network accounts for implication between literals on the same variable:
/// `(X < 0) => (X < 1)` is implicit and always true.
/// Thus, with an explicit implication `(Y < 0) => (X < 0)`, the network is able to infer the following facts:
///  - `(Y < 0)  => (X < 0)`
///  - `(Y < 0)  => (X < 1)`
///  - `(Y < -1) => (X < 0)`
///  - `(Y < 0)  => (Y < 1)`   (directly accessible with [`Lit::entails`])
///
/// # Limitations
///
/// The network does not check contradiction in the form of `x => !x`.
/// It also does not check for edge duplication, which might a cause for inefficiencies.
///
/// # Example
/// ```
/// use aries::core::*;
/// use aries::core::literals::ImplicationGraph;
/// let mut set = ImplicationGraph::empty();
/// let v1 = VarRef::from_u32(3); // arbitrary variable
/// let v2 = VarRef::from_u32(4); // arbitrary variable
/// assert!(!set.implies(v1.leq(0), v2.leq(0)));
/// set.add_implication(v1.leq(0), v2.leq(0));
/// assert!(set.implies(v1.leq(0), v2.leq(0)));
/// assert!(set.implies(v1.leq(0), v2.leq(1)));
/// assert!(set.implies(v1.leq(-1), v2.leq(0)));
/// assert!(!set.implies(v1.leq(1), v2.leq(0)));
/// ```
#[derive(Default)]
pub struct ImplicationGraph {
    edges: Watches<Lit>,
    num_edges: usize,
    cache: CachedDFS,
}

impl ImplicationGraph {
    /// Creates an empty implication graph
    pub fn empty() -> Self {
        Self::default()
    }

    /// Record the fact that `from` implies `to`.
    pub fn add_implication(&mut self, from: Lit, to: Lit) {
        if to == Lit::TRUE || from == Lit::FALSE || from.entails(to) {
            return;
        }
        self.num_edges += 1;
        self.edges.add_watch(to, from);
        self.edges.add_watch(!from, !to);
        self.cache.clear();
        debug_assert!(self.implies(from, to));
        debug_assert!(self.implies(!to, !from));
    }

    /// Return true if there is a direct or indirect implication `x => y`.
    pub fn implies(&self, x: Lit, y: Lit) -> bool {
        if y == Lit::TRUE || x == Lit::FALSE || x.entails(y) {
            return true;
        }
        if self.edges.watches_on(!y).next().is_none() {
            // fail fast: no incoming edges to y, which thus is not reachable
            // this is possible to check because, for each  (x -> y) edge we have a (!y -> !x) edge.
            return false;
        }
        // starting from `x`, do a depth first search in the implication graph,
        // looking for a literal that entails `y`

        self.cache.reachable(x, y, &self.edges)
    }

    pub fn direct_implications_of(&self, lit: Lit) -> impl Iterator<Item = Lit> + '_ {
        self.edges.watches_on(lit)
    }
}

impl Clone for ImplicationGraph {
    fn clone(&self) -> Self {
        ImplicationGraph {
            edges: self.edges.clone(),
            num_edges: self.num_edges,
            cache: Default::default(),
        }
    }
}

/// A cache of the least recently used DFS states. This reduces the cost of two subsequent reachability queries from the same source.
struct CachedDFS {
    cached_states: Mutex<lru::LruCache<Lit, DFSState>>,
}

impl Default for CachedDFS {
    fn default() -> Self {
        Self {
            cached_states: Mutex::new(lru::LruCache::new(NonZeroUsize::new(10).unwrap())),
        }
    }
}

impl CachedDFS {
    /// Returns true if source literal is reachable (implies) the target one in the graph induces by `edges`.
    /// Some intermediate computations are cached so the cache should be cleared if the edges have changed since the last invocation
    pub fn reachable(&self, source: Lit, target: Lit, edges: &Watches<Lit>) -> bool {
        if let Ok(ref mut mutex) = self.cached_states.try_lock() {
            mutex
                .get_or_insert_mut(source, || DFSState::new(source))
                .reachable(target, edges)
        } else {
            // could not get a lock on the cache, just proceed
            DFSState::new(source).reachable(target, edges)
        }
    }

    /// Clear any cache result.
    pub fn clear(&mut self) {
        self.cached_states.lock().unwrap().clear()
    }
}

/// State of an ongoing DFS
struct DFSState {
    /// Set of visited vertices
    visited: LitSet,
    /// Queue of vertices to visit next
    queue: Vec<Lit>,
}
impl DFSState {
    /// Initializes a new DFS from the source.
    pub fn new(source: Lit) -> Self {
        let mut state = DFSState {
            visited: LitSet::with_capacity(64),
            queue: Vec::with_capacity(64),
        };
        state.queue.push(source);
        state
    }

    /// Returns true if the target literal is reachable from the source.
    /// Extending the search until this can be proved or refuted.
    pub fn reachable(&mut self, target: Lit, edges: &Watches<Lit>) -> bool {
        if self.visited.contains(target) {
            return true;
        }
        // dfs through implications
        while let Some(curr) = self.queue.pop() {
            // to ensure correctness if the search proceeds again, we need to add all elements
            for next in edges.watches_on(curr) {
                if !self.visited.contains(next) {
                    self.queue.push(next);
                    self.visited.insert(next);
                }
            }
            // the state is clean for possibly continuing, check if we can stop immediately
            if curr.entails(target) {
                return true;
            }
        }
        debug_assert!(self.queue.is_empty() && !self.visited.contains(target));
        false
    }
}

#[cfg(test)]
mod test {
    use crate::core::literals::ImplicationGraph;
    use crate::core::*;

    const A: VarRef = VarRef::from_u32(0);
    const B: VarRef = VarRef::from_u32(1);
    const C: VarRef = VarRef::from_u32(2);
    const D: VarRef = VarRef::from_u32(3);

    #[test]
    fn test_implications() {
        let mut g = ImplicationGraph::empty();

        assert!(g.implies(A.leq(0), A.leq(0)));
        assert!(g.implies(A.leq(0), A.leq(1)));
        assert!(!g.implies(A.leq(0), B.leq(0)));
        assert!(!g.implies(A.leq(0), A.leq(-1)));

        g.add_implication(A.leq(1), B.leq(1));
        assert!(g.implies(A.leq(1), B.leq(1)));
        assert!(g.implies(A.leq(0), B.leq(1)));
        assert!(g.implies(A.leq(1), B.leq(2)));
        assert!(g.implies(A.leq(0), B.leq(2)));
        assert!(!g.implies(A.leq(1), B.leq(0)));
        assert!(!g.implies(A.leq(1), B.leq(0)));

        g.add_implication(B.leq(2), C.leq(2));
        assert!(g.implies(A.leq(1), B.leq(1)));
        assert!(g.implies(A.leq(1), C.leq(2)));
        assert!(g.implies(A.leq(1), C.leq(3)));
        assert!(!g.implies(A.leq(1), C.leq(1)));
        assert!(g.implies(A.leq(0), C.leq(2)));
        assert!(!g.implies(A.leq(2), C.leq(2)));
    }

    #[test]
    fn test_implication_cycle() {
        let mut g = ImplicationGraph::empty();

        g.add_implication(A.leq(0), B.leq(0));
        g.add_implication(B.leq(0), A.leq(0));

        g.add_implication(C.leq(0), D.leq(0));
        g.add_implication(D.leq(0), C.leq(0));

        assert!(!g.implies(A.leq(0), C.leq(0)))
    }
}
