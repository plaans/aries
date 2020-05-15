use crate::planning::state::{State, Operators, Op, Lit};
use crate::planning::ref_store::RefStore;

pub type Cost = u64;
const INFTY: Cost = 2^50;

pub trait ApplicableOperators {
    fn applicable_operators(&self) -> &[Op];
}
pub trait ConjunctiveCost {
    fn conjunction_cost(&self, conjunction: &[Lit]) -> Cost;
}

pub struct HAddResult {
    op_costs: RefStore<Op, Cost>,
    lit_costs: RefStore<Lit, Cost>,
    applicable: Vec<Op>
}

impl ApplicableOperators for HAddResult {
    fn applicable_operators(&self) -> &[Op] {
        self.applicable.as_slice()
    }
}
impl ConjunctiveCost for HAddResult {
    fn conjunction_cost(&self, conjunction: &[Lit]) -> Cost {
        conjunction.iter().map(move |&l| self.lit_costs[l]).sum()
    }
}

pub fn hadd(state: &State, ops: &Operators) -> HAddResult {
    let mut op_costs = RefStore::initialized(ops.len(), INFTY);
    let mut update = RefStore::initialized(ops.len(), false);
    for op in ops.iter() {
        if ops.preconditions(op).is_empty() {
            update[op] = true;
        }
    }

    let mut lit_costs = RefStore::initialized(state.len() * 2, INFTY);
    for lit in state.literals() {
        lit_costs[lit] = 0;
        for &a in ops.dependent_on(lit) {
            update[a] = true;
        }
    }

    let mut applicable = Vec::with_capacity(32);
    let mut again = true;
    while again {
        again = false;
        for op in ops.iter() {
            if update[op] {
                update[op] = false;
                let c: u64 = ops.preconditions(op).iter().map(|&lit| lit_costs[lit]).sum();
                if c < op_costs[op] {
                    op_costs[op] = c;
                    if c == 0 {
                        applicable.push(op);
                    }
                    for &p in ops.effects(op) {
                        if c + 1 < lit_costs[p] {
                            lit_costs[p] = c + 1;
                        }
                        for &a in ops.dependent_on(p) {
                            again = true;
                            update[a] = true;
                        }
                    }
                }
            }
        }
    }
    HAddResult {
        op_costs,
        lit_costs,
        applicable
    }
}