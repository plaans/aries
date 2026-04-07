use crate::{model::lang::Atom, prelude::*};

pub trait Dom {
    fn upper_bound(&self, svar: SignedVar) -> IntCst;

    fn presence(&self, var: VarRef) -> Lit;

    fn lower_bound(&self, svar: SignedVar) -> IntCst {
        -self.upper_bound(-svar)
    }
}

impl<X: ?Sized> Dom for &X
where
    X: Dom,
{
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        Dom::upper_bound(*self, svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        Dom::presence(*self, var)
    }
}
impl<X: ?Sized> Dom for &mut X
where
    X: Dom,
{
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        Dom::upper_bound(*self, svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        Dom::presence(*self, var)
    }
}

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
        dom.upper_bound(*self)
    }

    #[inline]
    fn lower_bound(&self, dom: impl Dom) -> Self::Value {
        dom.lower_bound(*self)
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

pub trait Boundable {
    type Value;
    fn leq(&self, ub: Self::Value) -> Lit;
    fn geq(&self, lb: Self::Value) -> Lit;
}

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

/// An expression that is a view of exactly one variable (which may be the [`VarRef::ZERO`] variable).
///
/// Notably implemented for `VarRef`, `Lit`, `IVar`, `SVar`, `BVar`
pub trait Term {
    /// Extracts the underlying variable in the expression.
    ///
    /// Note that the resulting in [`VarRef`] cannot in general be considered as equivalent to the expression.
    fn variable(self) -> VarRef;
}
impl Term for Lit {
    fn variable(self) -> VarRef {
        self.variable()
    }
}
impl Term for SignedVar {
    fn variable(self) -> VarRef {
        self.variable()
    }
}
impl<T: Into<VarRef>> Term for T {
    fn variable(self) -> VarRef {
        self.into()
    }
}
impl Term for IAtom {
    fn variable(self) -> VarRef {
        self.var.variable()
    }
}
impl Term for Atom {
    fn variable(self) -> VarRef {
        self.variable()
    }
}
