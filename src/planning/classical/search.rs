use crate::planning::classical::heuristics::*;
use crate::planning::classical::state::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

struct Node {
    s: State,
    plan: Vec<Op>,
    f: Cost,
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

impl Eq for Node {}

const WEIGHT: Cost = 3;

pub fn plan_search(initial_state: &State, ops: &Operators, goals: &[Lit]) -> Option<Vec<Op>> {
    let mut heap = BinaryHeap::new();
    let mut closed = HashSet::new();

    // initialize the priority queue with the initiale state
    let init = Node {
        s: initial_state.clone(),
        plan: Vec::new(),
        f: 0,
    };
    heap.push(init);

    // keep expanding the search tree until the priority queue is empty
    while let Some(n) = heap.pop() {
        if closed.contains(&n.s) {
            // we already visited this state, skip it.
            continue;
        }

        // note that we are visiting this state to avoid going back to it
        closed.insert(n.s.clone());

        // compute hadd to get the set of applicable actions
        // TODO: there is a lot of costly unnecessary work here
        let hres = hadd(&n.s, ops);

        // for each operator applicable in the current state
        for &op in hres.applicable_operators() {
            debug_assert!(n.s.entails_all(ops.preconditions(op)));

            // clone the state and apply effects
            let mut s = n.s.clone();
            s.set_all(ops.effects(op));

            // create the corresponding plan
            let mut plan = n.plan.clone();
            plan.push(op);

            // check if we have reached a goal state
            if s.entails_all(goals) {
                return Some(plan);
            }

            // compute heuristic value and add to queue
            let hres = hadd(&s, ops);
            let f = (plan.len() as Cost) + 3 * hres.conjunction_cost(goals);
            let succ = Node { s, plan, f };
            heap.push(succ);
        }
    }

    // we have exausted the search space without finding a goal state, problem in unsolvable
    None
}
