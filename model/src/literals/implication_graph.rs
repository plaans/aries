use crate::literals::{Lit, LitSet, Watches};

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
/// use aries_model::literals::{LitSet, ImplicationGraph};
/// use aries_model::lang::VarRef;
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
#[derive(Clone, Default)]
pub struct ImplicationGraph {
    edges: Watches<Lit>,
    num_edges: usize,
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

        // list of literals that were previously encountered in search
        let mut visited = LitSet::with_capacity(64);
        let mut queue = Vec::with_capacity(64);
        queue.push(x);

        // dfs through implications
        while let Some(curr) = queue.pop() {
            if visited.contains(curr) {
                continue;
            }
            visited.insert(curr);
            for next in self.edges.watches_on(curr) {
                if next.entails(y) {
                    return true;
                } else {
                    queue.push(next);
                }
            }
        }
        false
    }

    pub fn direct_implications_of(&self, lit: Lit) -> impl Iterator<Item = Lit> + '_ {
        self.edges.watches_on(lit)
    }
}

#[cfg(test)]
mod test {
    use crate::lang::VarRef;
    use crate::literals::ImplicationGraph;

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
