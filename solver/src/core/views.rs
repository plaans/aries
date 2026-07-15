mod dom;

pub use dom::Dom;

use crate::prelude::*;

pub trait VarView {
    type Value: Ord;

    fn upper_bound(&self, dom: impl Dom) -> Self::Value;
    fn lower_bound(&self, dom: impl Dom) -> Self::Value;
}

impl<T: VarView> VarView for &T {
    type Value = <T as VarView>::Value;

    fn upper_bound(&self, dom: impl Dom) -> Self::Value {
        VarView::upper_bound(*self, dom)
    }

    fn lower_bound(&self, dom: impl Dom) -> Self::Value {
        VarView::lower_bound(*self, dom)
    }
}

impl VarView for SignedVar {
    type Value = IntCst;

    #[inline]
    fn upper_bound(&self, dom: impl Dom) -> Self::Value {
        dom._upper_bound(*self)
    }

    #[inline]
    fn lower_bound(&self, dom: impl Dom) -> Self::Value {
        dom._lower_bound(*self)
    }
}

impl VarView for Lit {
    type Value = bool;

    #[inline]
    fn upper_bound(&self, dom: impl Dom) -> Self::Value {
        // the upper bound is `false` if the Lit is not satisfiable anymore
        // i.e.,  given the lit `(var <= val)`  it is incompatible with the lower bound var >= val +1
        self.svar().lower_bound(dom) <= self.ub_value()
    }

    #[inline]
    fn lower_bound(&self, dom: impl Dom) -> Self::Value {
        // the lower bound is `true` if the Lit is always satified
        // i.e.,  given the lit `(var <= val)`  it is satisfied if ub(var) <= val
        self.svar().upper_bound(dom) <= self.ub_value()
    }
}

/// A variable for which setting an upper or lower bound bound can be represented as a [`Lit`].
pub trait Boundable {
    /// Type of the values taken by the variable.
    type Value;

    /// Literal equivalent to a lower bound on the variable.
    fn leq(&self, ub: Self::Value) -> Lit;

    /// Literal equivalent to an upper bound on the variable.
    fn geq(&self, lb: Self::Value) -> Lit;
}

/// A [`Boundable`] variable with values of type [`IntCst`].
///
/// The trait provides additional convenience methods for this common case.
pub trait IntBoundable: Boundable<Value = IntCst> {
    /// Literal equivalent to a strict upper bound on the variable.
    #[inline]
    fn lt(&self, ub: Self::Value) -> Lit {
        self.leq(ub - 1)
    }

    /// Literal equivalent to a strict lower bound on the variable.
    #[inline]
    fn gt(&self, lb: Self::Value) -> Lit {
        self.geq(lb + 1)
    }

    /// Signed variable that would appear in the [`Lit`] when requesting an upper bound.
    fn upper_bounding_signed_var(&self) -> SignedVar {
        self.leq(0).svar()
    }

    /// Signed variable that would appear in the [`Lit`] when requesting an lower bound.
    fn lower_bounding_signed_var(&self) -> SignedVar {
        self.geq(0).svar()
    }
}

impl<T> IntBoundable for T where T: Boundable<Value = IntCst> {}

impl Boundable for SignedVar {
    type Value = IntCst;

    #[inline]
    fn leq(&self, ub: Self::Value) -> Lit {
        Lit::leq(*self, ub)
    }

    #[inline]
    fn geq(&self, lb: Self::Value) -> Lit {
        Lit::geq(*self, lb)
    }
}

/// Determine whether an expression is already determined to be present in the given domain..
///
/// If an expression is always defined (its presence literal is [`Lit::TRUE`]), it should return `true` regardless of the domain.
pub trait Optional {
    fn present(&self, domains: impl Dom) -> bool;
}

impl<T: Term + Copy> Optional for T {
    fn present(&self, domains: impl Dom) -> bool {
        domains.present(self.variable()) == Some(true)
    }
}

/// An expression that is a view of exactly one variable (which may be the [`Var::ZERO`] variable).
///
/// Notably implemented for  [`Var`], [`Lit`], [`VarCst`]
pub trait Term {
    /// Extracts the underlying variable in the expression.
    ///
    /// Note that the resulting in [`Var`] cannot in general be considered as equivalent to the expression.
    fn variable(self) -> Var;
}
impl Term for Lit {
    fn variable(self) -> Var {
        self.variable()
    }
}
impl Term for SignedVar {
    fn variable(self) -> Var {
        self.variable()
    }
}
impl<T: Into<Var>> Term for T {
    fn variable(self) -> Var {
        self.into()
    }
}
impl Term for VarCst {
    fn variable(self) -> Var {
        self.var.variable()
    }
}
