use crate::{lang::ModelView, prelude::*};

/// Allows explicitly setting the *scope* of a constraint.
///
/// The newly imposed scoped must be at least as strict as the original.
/// For instance, for a constraint `C` that is defined when `x` and `y` and entailed:
///  - it is legal to set its scope to `x` `y` and `z` (`c.scoped([x, y, z])`)
///  - it is forbidden to set its scope to `x` only (`c.scoped([x])`) because it would make the constraint
///    evaluable in contexts where is it not defined
///  - it is a no-op to set its scope to `x` and `y`
///
/// The typical use-case of explicit scoping is when constants (that are by definition always defined)
/// are logically part of a optional construct. For instance the constant duration of an optional task.
/// All constraint on these elements should be logically have the same scope as the optional task.
///
/// The [`ScopedExt`] trait (exported in prelude) provides extension syntax for scoping a constraint.
///
/// ```
/// use aries_solver::prelude::*;
/// let mut model = Model::new();
/// // create a scope variable
/// let scope = model.new_bool_var();
///
/// // create an optional variable that is online present when `scope` is entailed.
/// let duration: VarCst = model.new_optional_variable(6, 6, scope).into();
/// // here duration is an optional variable so the constraint is only defined (in scope) where `scope` is entailed
/// let constraint = leq(duration, 5);
/// // enforcing the constraint would only enforce it when it is in scope
/// model.enforce(constraint);
/// // model.enforce(constraint.scoped(scope)); // equivalent to the above
///
/// // ALTERNATIVE ENCODING, where the duration is represented by a constant, which requires scoping the constraint.
/// // if we were to replace the variable by its constant value, we would loose the scope information.
/// // (6 <= 5) being always false, the model would be unsatisfiable
/// let duration: VarCst = 6.into();
/// let constraint = leq(duration, 5);
/// // posting the constraint with an explicit scope `.scoped()` replaces it in same context as the one above
/// model.enforce(constraint.scoped(scope));
///
/// ```
#[derive(Debug, Clone)]
pub struct Scoped<Constraint> {
    constraint: Constraint,
    scope: Conjunction,
}

impl<Constraint> Scoped<Constraint> {
    /// Creates a new version of `constraint` as is seen as only defined ("in scope") when all literals of `scope` are entailed.
    pub fn new(constraint: Constraint, scope: Conjunction) -> Self {
        Self { constraint, scope }
    }
}

impl<Ctx: ModelView, Constraint: BoolExpr<Ctx>> BoolExpr<Ctx> for Scoped<Constraint> {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        self.constraint.enforce_if(implicant, ctx);
    }

    fn conj_scope(&self, _ctx: &Ctx) -> Conjunction {
        self.scope.clone()
    }
}

/// Provides extension syntax for scoping a constraint.
pub trait ScopedExt {
    /// Explicitly sets the *scope* of a constraint.
    ///
    /// The resulting constraint will be interpreted as only in defined ("in scope") when all literals of `scope` are true.
    fn scoped(self, scope: impl Into<Conjunction>) -> Scoped<Self>
    where
        Self: Sized;
}

impl<T> ScopedExt for T {
    fn scoped(self, scope: impl Into<Conjunction>) -> Scoped<Self>
    where
        Self: Sized,
    {
        Scoped::new(self, scope.into())
    }
}
