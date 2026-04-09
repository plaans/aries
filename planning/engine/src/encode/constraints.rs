use aries::{
    model::lang::{BoolExpr, expr::or},
    prelude::*,
};
use timelines::{IntExp, constraints::HasValueAt, encoder::SchedEncoder};

/// Constraint representing a condition
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
            ConditionConstraint::EqZero(sum) => {
                // TODO: suboptimal in many cases. Need special handling in solver
                sum.clone().leq(0).enforce_if(l, ctx);
                sum.clone().geq(0).enforce_if(l, ctx);
            }
            ConditionConstraint::NeqZero(sum) => {
                // TODO: suboptimal in many cases. Need special handling in solver
                let greater = sum.clone().geq(1).implicant(ctx); // TODO: 1 will ony work for integers
                let smaller = sum.clone().leq(-1).implicant(ctx); // TODO: -1 will ony work for integers
                or([greater, smaller]).enforce_if(l, ctx);
            }
            ConditionConstraint::LeqZero(sum) => sum.clone().leq(0).enforce_if(l, ctx),
        }
    }

    fn conj_scope(&self, ctx: &SchedEncoder) -> Conjunction {
        match self {
            ConditionConstraint::HasValue(has_value_at) => has_value_at.conj_scope(ctx),
            ConditionConstraint::EqZero(sum) => sum.clone().leq(INT_CST_MAX).conj_scope(ctx), // TODO: improve
            ConditionConstraint::NeqZero(sum) => sum.clone().leq(INT_CST_MAX).conj_scope(ctx),
            ConditionConstraint::LeqZero(linear_sum) => linear_sum.clone().leq(0).conj_scope(ctx),
        }
    }
}
