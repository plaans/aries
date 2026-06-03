use crate::{model::lang::Store, prelude::*};

/// Represents an integer expression that can be reified or made equal to another variable
pub trait IntExpr<Ctx: Store> {
    /// Reifies the expression into a term with a single variable.
    fn reify(&self, scope: impl Into<Conjunction>, ctx: &mut Ctx) -> LinTerm {
        let scope = scope.into();
        let scope = ctx.conjunctive_scope(scope.literals());
        let (lb, ub) = self.bounds(ctx);
        let reif = ctx.new_optional_var(lb, ub, scope);
        let reif = LinTerm::from(reif);
        self.enforce_eq(reif, ctx);
        reif
    }

    /// Returns the lower and upper bounds of the expressions.
    ///
    /// The default implementation just returns the lower bound [`INT_CST_MIN`] and the upper bound  [`INT_CST_MAX`].`
    fn bounds(&self, _ctx: &Ctx) -> (IntCst, IntCst) {
        (INT_CST_MIN, INT_CST_MAX)
    }

    /// Attempts to provide a linearization of the expression. It must be the case that the expression is defined within the provided scope.
    ///
    /// The default implementation will simply reify the expression to a single variable and return the [`LinSum`] with only this variable]
    fn linearize(&self, scope: impl Into<Conjunction>, ctx: &mut Ctx) -> LinSum {
        self.reify(scope, ctx).into()
    }

    /// Enforce that, whenever `variable` is defined, it has the same value as the expression.
    fn enforce_eq(&self, variable: LinTerm, ctx: &mut Ctx) {
        // Create an enabler that is always true and has the same scope as `variable`
        let enabler = ctx.tautology_of_scope(ctx.presence_literal(variable));
        self.enforce_eq_if(enabler, variable, ctx);
    }

    /// Enforces that when `implicant` is true and defined,
    fn enforce_eq_if(&self, implicant: Lit, variable: LinTerm, ctx: &mut Ctx);
}
