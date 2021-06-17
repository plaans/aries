use crate::bounds::{Bound, Watches};
use std::collections::VecDeque;

#[derive(Clone, Default)]
pub struct TwoSatTree {
    edges: Watches<Bound>,
    num_edges: usize,
}

impl TwoSatTree {
    pub fn add_implication(&mut self, from: Bound, to: Bound) {
        debug_assert!(
            from.variable() > to.variable(),
            "Invariant that should be maintained by OptDomains {:?}  {:?}",
            from,
            to
        );
        debug_assert!(!self.implies(to, from));
        self.num_edges += 1;
        self.edges.add_watch(to, from);
        self.edges.add_watch(!from, !to);
        debug_assert!(self.implies(from, to));
        debug_assert!(self.implies(!to, !from));
    }

    pub fn implies(&self, x: Bound, y: Bound) -> bool {
        // TODO: we can be smarter and only go up the tree.
        //       key observation: by construction, the variable ID of a node is strictly greater than
        //       the one of its ancestor.
        let mut num_iter = 0;
        let mut queue = VecDeque::new();
        queue.push_back(x);
        // dfs through implications
        while let Some(curr) = queue.pop_front() {
            for next in self.edges.watches_on(curr) {
                if next.entails(y) {
                    return true;
                } else {
                    queue.push_back(next);
                }
            }
            debug_assert!(num_iter <= self.num_edges, "The structure appear not to be a tree");
            num_iter += 1;
        }
        false
    }

    pub fn direct_implications_of(&self, lit: Bound) -> impl Iterator<Item = Bound> + '_ {
        self.edges.watches_on(lit)
    }
}
