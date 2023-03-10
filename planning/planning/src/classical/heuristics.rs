use crate::classical::state::{Lit, Op, Operators, State};
use aries::collections::ref_store::RefStore;

/// Representation of the cost to achieve a literal or action.
/// Having an infinite cost implies that the item can not appear in any solution plan.
pub type Cost = f32;

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
    /// Provides an estimation of the cost of the operator.
    /// THe cost is infinite provably impossible.
    fn operator_cost(&self, op: Op) -> Cost;
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
        let x = self.lit_costs[literal];
        debug_assert!(!x.is_nan());
        x
    }
    fn conjunction_cost(&self, conjunction: &[Lit]) -> Cost {
        conjunction.iter().map(|&lit| self.literal_cost(lit)).sum()
    }
}
impl OperatorCost for HAddResult {
    fn operator_cost(&self, op: Op) -> Cost {
        let x = self.op_costs[op];
        debug_assert!(!x.is_nan());
        x
    }
}

pub fn hadd(state: &State, ops: &Operators) -> HAddResult {
    let mut op_costs = RefStore::initialized(ops.size(), Cost::INFINITY);
    let mut update = RefStore::initialized(ops.size(), false);
    for op in ops.iter() {
        if ops.preconditions(op).is_empty() {
            update[op] = true;
        }
    }

    let mut lit_costs = RefStore::initialized(state.num_variables() * 2, Cost::INFINITY);
    for lit in state.literals() {
        lit_costs[lit] = 0.;
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
                let c: Cost = ops.preconditions(op).iter().map(|&lit| lit_costs[lit]).sum();
                if c < op_costs[op] {
                    op_costs[op] = c;
                    if c == 0. {
                        applicable.push(op);
                    }
                    for &p in ops.effects(op) {
                        if c + 1. < lit_costs[p] {
                            lit_costs[p] = c + 1.;
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
