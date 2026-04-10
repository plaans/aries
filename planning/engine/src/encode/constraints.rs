use aries::{model::lang::BoolExpr, prelude::*};
use timelines::{IntExp, constraints::HasValueAt, encoder::SchedEncoder};

/// Constraint representing a condition
#[derive(Debug)]
pub enum ConditionConstraint {
    HasValue(HasValueAt),
    EqZero(IntExp),
    NeqZero(IntExp),
    LeqZero(IntExp),
}
impl BoolExpr<SchedEncoder> for ConditionConstraint {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoder) {
        match self {
            ConditionConstraint::HasValue(has_value_at) => has_value_at.enforce_if(l, ctx),
            ConditionConstraint::EqZero(sum) => sum.clone().eq(0).enforce_if(l, ctx),
            ConditionConstraint::NeqZero(sum) => sum.clone().neq(0).enforce_if(l, ctx),
            ConditionConstraint::LeqZero(sum) => sum.clone().leq(0).enforce_if(l, ctx),
        }
    }

    fn conj_scope(&self, ctx: &SchedEncoder) -> Conjunction {
        use ConditionConstraint::*;
        match self {
            HasValue(has_value_at) => has_value_at.conj_scope(ctx),
            EqZero(sum) | NeqZero(sum) | LeqZero(sum) => sum.conj_scope(ctx),
        }
    }
}
