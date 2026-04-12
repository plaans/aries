use aries::{core::views::Dom, model::lang::BoolExpr, prelude::*};
use timelines::{IntExp, constraints::HasValueAt, encoder::SchedEncoder};

use crate::encode::required_values::RequiredValues;

#[derive(Debug)]
pub struct ConditionConstraint {
    pub constraint: ConditionExpression,
    pub scope: Lit,
}

/// Constraint representing a condition
#[derive(Debug)]
pub enum ConditionExpression {
    HasValue(HasValueAt),
    EqZero(IntExp),
    NeqZero(IntExp),
    LeqZero(IntExp),
    Or(Vec<ConditionConstraint>),
    And(Vec<ConditionConstraint>),
}

impl ConditionExpression {
    pub fn scoped(self, scope: Lit) -> ConditionConstraint {
        ConditionConstraint {
            constraint: self,
            scope,
        }
    }

    pub fn add_required_values(&self, required_values: &mut RequiredValues, model: &planx::Model, dom: impl Dom) {
        use ConditionExpression::*;
        match &self {
            HasValue(c) => {
                // record that someone required such a value
                let fluent_id = model.env.fluents.get_by_name(&c.state_var.fluent).unwrap();
                required_values.add(fluent_id, c.value_box(&dom).as_ref());
            }
            EqZero(_) | NeqZero(_) | LeqZero(_) | Or(_) | And(_) => {} // already tracked when parsing.
        }
    }
}

impl std::ops::Not for ConditionExpression {
    type Output = ConditionExpression;

    fn not(self) -> Self::Output {
        match self {
            ConditionExpression::HasValue(mut c) => {
                // TODO: this only works for booleans
                if let Ok(x) = IntCst::try_from(c.value)
                    && (x == 0 || x == 1)
                {
                    c.value = (1 - x).into(); // negation : 0 -> 1 and 1 -> 0
                    ConditionExpression::HasValue(c)
                } else {
                    // normally this add value only arises when with an effect on a predicate
                    // or its negation so this should never occur but it should be better captured by the type system
                    unreachable!()
                }
            }
            ConditionExpression::EqZero(sum) => ConditionExpression::NeqZero(sum),
            ConditionExpression::NeqZero(sum) => ConditionExpression::EqZero(sum),
            // !(sum <= 0) <=> (sum > 0) <=> (-sum < 0) <=> -sum <= -1 <=> -sum +1 <= 0
            ConditionExpression::LeqZero(sum) => ConditionExpression::LeqZero(-sum + 1),
            ConditionExpression::Or(disjuncts) => ConditionExpression::And(disjuncts.into_iter().map(|d| !d).collect()),
            ConditionExpression::And(conjuncts) => ConditionExpression::Or(conjuncts.into_iter().map(|d| !d).collect()),
        }
    }
}

impl std::ops::Not for ConditionConstraint {
    type Output = ConditionConstraint;

    fn not(self) -> Self::Output {
        ConditionConstraint {
            constraint: !self.constraint,
            scope: self.scope,
        }
    }
}

impl BoolExpr<SchedEncoder> for ConditionConstraint {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoder) {
        match &self.constraint {
            ConditionExpression::HasValue(has_value_at) => has_value_at.enforce_if(l, ctx),
            ConditionExpression::EqZero(sum) => sum.clone().eq(0).enforce_if(l, ctx),
            ConditionExpression::NeqZero(sum) => sum.clone().neq(0).enforce_if(l, ctx),
            ConditionExpression::LeqZero(sum) => sum.clone().leq(0).enforce_if(l, ctx),
            ConditionExpression::Or(condition_constraints) => {
                // enforce that at least one is present
                Disjunction::from_iter(condition_constraints.iter().map(|c| c.scope)).enforce_if(l, ctx);
                for c in condition_constraints {
                    // enforce that they are enforced if present
                    c.opt_enforce_if(l, ctx);
                }
            }
            ConditionExpression::And(cs) => {
                // if enforced all elements must be present
                Conjunction::from_iter(cs.iter().map(|c| c.scope)).enforce_if(l, ctx);
                for c in cs {
                    // constraint must hold if present
                    c.opt_enforce_if(l, ctx);
                }
            }
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> Conjunction {
        [self.scope].into()
    }
}
