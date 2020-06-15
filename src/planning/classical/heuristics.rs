use crate::planning::classical::state::{Lit, Op, Operators, State};
use crate::planning::ref_store::RefStore;

// TODO: make a proper implementation that guarantees no overflow and singles out infinite values
pub type Cost = u64;
pub const COST_INFTY: Cost = 2 ^ 50;

pub trait ApplicableOperators {
    fn applicable_operators(&self) -> &[Op];
}
pub trait LiteralCost {
    fn literal_cost(&self, literal: Lit) -> Cost;

    /// Cost of a conjunction of literals. Simple possibilities are to take the max (for hmax) or the
    /// sum (for hadd) of the individual costs.
    fn conjunction_cost(&self, conjunction: &[Lit]) -> Cost;
}
pub trait OperatorCost {
    /// Provides an estimation of the cost of the operator or None
    /// if the operator is provably impossible (has infinite cost)
    fn operator_cost(&self, op: Op) -> Option<Cost>;
}

pub struct HAddResult {
    op_costs: RefStore<Op, Cost>,
    lit_costs: RefStore<Lit, Cost>,
    applicable: Vec<Op>,
}

impl ApplicableOperators for HAddResult {
    fn applicable_operators(&self) -> &[Op] {
        self.applicable.as_slice()
    }
}
impl LiteralCost for HAddResult {
    fn literal_cost(&self, literal: Lit) -> Cost {
        self.lit_costs[literal]
    }
    fn conjunction_cost(&self, conjunction: &[Lit]) -> Cost {
        conjunction.iter().map(|&lit| self.literal_cost(lit)).sum()
    }
}
impl OperatorCost for HAddResult {
    fn operator_cost(&self, op: Op) -> Option<u64> {
        let c = self.op_costs[op];
        if c >= COST_INFTY {
            None
        } else {
            Some(c)
        }
    }
}

pub fn hadd(state: &State, ops: &Operators) -> HAddResult {
    let mut op_costs = RefStore::initialized(ops.size(), COST_INFTY);
    let mut update = RefStore::initialized(ops.size(), false);
    for op in ops.iter() {
        if ops.preconditions(op).is_empty() {
            update[op] = true;
        }
    }

    let mut lit_costs = RefStore::initialized(state.size() * 2, COST_INFTY);
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
        applicable,
    }
}
