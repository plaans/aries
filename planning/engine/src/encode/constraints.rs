use aries::{
    model::lang::{
        expr::{eq, neq},
        hreif::{BoolExpr, Store},
    },
    prelude::*,
};
use timelines::{Sched, constraints::HasValueAt};

/// Constraint representing a condition
pub enum ConditionConstraint {
    HasValue(HasValueAt),
    Eq(IAtom, IAtom),
    Neq(IAtom, IAtom),
}
impl BoolExpr<Sched> for ConditionConstraint {
    fn enforce_if(&self, l: Lit, ctx: &Sched, store: &mut dyn Store) {
        match self {
            ConditionConstraint::HasValue(has_value_at) => has_value_at.enforce_if(l, ctx, store),
            ConditionConstraint::Eq(a, b) => eq(*a, *b).enforce_if(l, ctx, store),
            ConditionConstraint::Neq(a, b) => neq(*a, *b).enforce_if(l, ctx, store),
        }
    }

    fn conj_scope(&self, ctx: &Sched, store: &dyn Store) -> aries::model::lang::hreif::Lits {
        match self {
            ConditionConstraint::HasValue(has_value_at) => has_value_at.conj_scope(ctx, store),
            ConditionConstraint::Eq(a, b) => eq(*a, *b).conj_scope(ctx, store),
            ConditionConstraint::Neq(a, b) => neq(*a, *b).conj_scope(ctx, store),
        }
    }
}
