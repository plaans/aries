use aries::{
    model::lang::{
        expr::or,
        hreif::{BoolExpr, Store},
        linear::LinearSum,
    },
    prelude::*,
};
use timelines::{Sched, constraints::HasValueAt};

/// Constraint representing a condition
pub enum ConditionConstraint {
    HasValue(HasValueAt),
    EqZero(LinearSum),
    NeqZero(LinearSum),
    LeqZero(LinearSum),
}
impl BoolExpr<Sched> for ConditionConstraint {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        match self {
            ConditionConstraint::HasValue(has_value_at) => has_value_at.enforce_if(l, ctx, store),
            ConditionConstraint::EqZero(sum) => {
                // TODO: suboptimal in many cases. Need special handling in solver
                sum.clone().leq(0).enforce_if(l, ctx, store);
                sum.clone().geq(0).enforce_if(l, ctx, store);
            }
            ConditionConstraint::NeqZero(sum) => {
                // TODO: suboptimal in many cases. Need special handling in solver
                let greater = sum.clone().geq(1).implicant(ctx, store); // TODO: 1 will ony work for integers
                let smaller = sum.clone().leq(-1).implicant(ctx, store); // TODO: -1 will ony work for integers
                or([greater, smaller]).enforce_if(l, ctx, store);
            }
            ConditionConstraint::LeqZero(sum) => sum.clone().leq(0).enforce_if(l, ctx, store),
        }
    }

    fn conj_scope(&self, ctx: &Sched, store: &dyn Store) -> aries::model::lang::hreif::Lits {
        match self {
            ConditionConstraint::HasValue(has_value_at) => has_value_at.conj_scope(ctx, store),
            ConditionConstraint::EqZero(sum) => sum.clone().leq(INT_CST_MAX).conj_scope(ctx, store), // TODO: improve
            ConditionConstraint::NeqZero(sum) => sum.clone().leq(INT_CST_MAX).conj_scope(ctx, store),
            ConditionConstraint::LeqZero(linear_sum) => linear_sum.clone().leq(0).conj_scope(ctx, store),
        }
    }
}
