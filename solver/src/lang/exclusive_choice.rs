use crate::{
    lang::{BoolExpr, Store, expr::or},
    prelude::*,
};

/// Builds an exclusive choice between two alternatives.
///
/// Excluvise choices are useful in that they sometime allow to avoid creating intermediate
/// variable (see [`ExclusiveChoice`]).
pub fn exclu_choice<T>(alt1: T, alt2: T) -> ExclusiveChoice<T> {
    ExclusiveChoice { alt1, alt2 }
}

/// Represent a choice between two incompatible choices.
/// `ExclusiveChoice(a, b) <=> a or b` however it is in addition known
/// that  `(a -> !b) and (b -> !a)` (i.e. the two choices are mutually exclusive).
///
/// When enforced (half-reified to an always true literal),
/// we can thus create a single variable `l` and impose:
///   - `l -> a`
///   - `!l -> b`
pub struct ExclusiveChoice<T> {
    /// First alternative (exclusive to the second one)
    alt1: T,
    /// Second alternative (exclusive to the first one)
    alt2: T,
}

impl<Ctx: Store, T: BoolExpr<Ctx>> BoolExpr<Ctx> for ExclusiveChoice<T> {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        if ctx.entails(l) {
            // a tautolgy, create a single variable representing both options
            let choice_var = ctx.new_literal(ctx.presence_literal(l));
            self.alt1.opt_enforce_if(choice_var, ctx);
            self.alt2.opt_enforce_if(!choice_var, ctx);
        } else {
            // no optimisation possible, resort to general formulation
            let a = self.alt1.implicant(ctx);
            let b = self.alt2.implicant(ctx);
            or([a, b]).opt_enforce_if(l, ctx);
        }
    }
    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        let mut sa = self.alt1.conj_scope(ctx).into_lits();
        let sb = self.alt2.conj_scope(ctx);
        sa.extend_from_slice(&sb);
        Conjunction::new(sa)
    }
}
