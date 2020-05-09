

use crate::chronicles::state::*;
use crate::chronicles::heuristics::*;
use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;



struct Node {
    s: State,
    plan: Vec<Op>,
    f: Cost
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        Cost::cmp(&self.f, &other.f).reverse()
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Node {

}

const WEIGHT: Cost = 3;

pub fn plan_search(initial_state: &State, ops: &Operators, goals: &[Lit]) -> Option<Vec<Op>> {

    let mut heap = BinaryHeap::new();
    let mut closed = HashSet::new();

    let init = Node {
        s: initial_state.clone(),
        plan: Vec::new(),
        f: 0
    };
    heap.push(init);

    while let Some(n) = heap.pop() {
        if closed.contains(&n.s) {
            continue
        }
        closed.insert(n.s.clone());
        let hres = hadd(&n.s, ops);
        for &op in hres.applicable_operators() {
            debug_assert!(n.s.entails_all(ops.preconditions(op)));
            let mut s = n.s.clone();
            s.set_all(ops.effects(op));

            let mut plan = n.plan.clone();
            plan.push(op);

            if s.entails_all(goals) {
                return Some(plan);
            }

            let hres = hadd(&s, ops);
            let f = (plan.len() as Cost) + 3 * hres.conjunction_cost(goals);
            let succ = Node { s, plan, f };
            heap.push(succ);
        }
    }

    None
}