use std::ops::Not;

use aries::{core::views::Dom, model::lang::BoolExpr, prelude::*};
use timelines::{IntExp, constraints::HasValueAt, encoder::SchedEncoder};

use crate::encode::required_values::RequiredValues;

#[derive(Clone, Debug)]
pub struct ConditionConstraint {
    pub constraint: ConditionExpression,
    pub scope: Lit,
}

impl ConditionConstraint {
    /// Records all fluents values that may be needed in evaluating the condition.
    /// This is used notably for pruning unecessary effects (in the closed world assumption step)
    pub fn add_required_values(&self, required_values: &mut RequiredValues, model: &planx::Model, dom: &impl Dom) {
        self.constraint.add_required_values(required_values, model, dom);
    }
}

/// Constraint representing a condition
#[derive(Clone, Debug)]
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

    /// Records all fluents values that may be needed in evaluating the condition.
    /// This is used notably for pruning unecessary effects (in the closed world assumption step)
    pub fn add_required_values(&self, required_values: &mut RequiredValues, model: &planx::Model, dom: &impl Dom) {
        use ConditionExpression::*;
        match &self {
            HasValue(c) => {
                // record that someone required such a value
                let fluent_id = model.env.fluents.get_by_name(&c.state_var.fluent).unwrap();
                required_values.add(fluent_id, c.value_box(dom).as_ref());
            }
            EqZero(_) | NeqZero(_) | LeqZero(_) => {} // already tracked when reifying the subexpressions (necessarily done when reifying)
            Or(subs) | And(subs) => {
                // add required values for all subexpressions
                subs.iter()
                    .for_each(|c| c.add_required_values(required_values, model, dom));
            }
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
                let _span = tracing::debug_span!("Or");
                let _span = _span.enter();
                // enforce that at least one is present
                Disjunction::from_iter(condition_constraints.iter().map(|c| c.scope)).enforce_if(l, ctx);
                for c in condition_constraints {
                    let _span = tracing::debug_span!("Disjunct");
                    let _span = _span.enter();
                    // enforce that they are enforced if present
                    c.opt_enforce_if(l, ctx);
                }
            }
            ConditionExpression::And(cs) => {
                let _span = tracing::debug_span!("And");
                let _span = _span.enter();
                // if enforced all elements must be present
                Conjunction::from_iter(cs.iter().map(|c| c.scope)).enforce_if(l, ctx);
                for c in cs {
                    let _span = tracing::debug_span!("Conjunct");
                    let _span = _span.enter();
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

/// A constraint that enforces a literal to be truee iff the associated expresison is true.
#[derive(Debug, Clone)]
pub struct ReificationConstraint {
    /// A literal that is made true if and only if the constraint holds
    pub reification: Lit,
    /// The constraint that is reified
    pub constraint: ConditionConstraint,
}

impl ReificationConstraint {
    /// Records all fluents values that may be needed in evaluating the condition.
    /// This is used notably for pruning unecessary effects (in the closed world assumption step)
    pub fn add_required_values(&self, required_values: &mut RequiredValues, model: &planx::Model, dom: &impl Dom) {
        // add the required values for both the original and its negation
        self.constraint.add_required_values(required_values, model, dom);
        self.constraint
            .clone()
            .not()
            .add_required_values(required_values, model, dom);
    }
}

impl BoolExpr<SchedEncoder> for ReificationConstraint {
    fn enforce_if(&self, implicant: Lit, ctx: &mut SchedEncoder) {
        let must_hold = Conjunction::from([implicant, self.reification]).reified(ctx);
        self.constraint.enforce_if(must_hold, ctx);
        let must_be_violated = Conjunction::from([implicant, !self.reification]).reified(ctx);
        (!self.constraint.clone()).enforce_if(must_be_violated, ctx);
    }

    fn conj_scope(&self, ctx: &SchedEncoder) -> Conjunction {
        ctx.presence_literal(self.reification).into()
    }
}
