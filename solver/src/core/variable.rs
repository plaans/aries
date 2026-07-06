use crate::{
    core::{
        IntCst, Lit, SignedVar,
        views::{Boundable, VarView},
    },
    create_ref_type,
};
use std::{fmt::Debug, hash::Hash};

/// Kept to ease the transition from `VarRef`
#[deprecated = "Use Var instead."]
#[doc(hidden)]
pub type VarRef = Var;

create_ref_type!(Var);

// Implement Debug for Var
// `?` represents a variable
impl Debug for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "var{:?}", self.to_u32())
    }
}

impl Var {
    /// A reserved special variable that is always equal to 0. It corresponds to the first representable Var.
    ///
    /// For efficiency reasons, this special case is not treated separately from the other variables, and it is the responsibility
    /// of the producers of Var to ensure that they only emit this value for variables whose domain is `[0,0]`.
    pub const ZERO: Var = Var::from_u32(0);

    /// A reserved special variable that is always equal to 1. It corresponds to the second representable Var.
    ///
    /// For efficiency reasons, this special case is not treated separately from the other variables, and it is the responsibility
    /// of the producers of Var to ensure that they only emit this value for variables whose domain is `[1,1]`.
    #[doc(hidden)]
    pub const ONE: Var = Var::from_u32(1);

    pub fn leq(self, i: IntCst) -> Lit {
        Lit::leq(self, i)
    }
    pub fn lt(self, i: IntCst) -> Lit {
        Lit::lt(self, i)
    }
    pub fn geq(self, i: IntCst) -> Lit {
        Lit::geq(self, i)
    }
    pub fn gt(self, i: IntCst) -> Lit {
        Lit::gt(self, i)
    }
}

impl VarView for Var {
    type Value = IntCst;

    fn upper_bound(&self, dom: impl super::views::Dom) -> Self::Value {
        dom.upper_bound(SignedVar::plus(*self))
    }

    fn lower_bound(&self, dom: impl super::views::Dom) -> Self::Value {
        dom.lower_bound(SignedVar::plus(*self))
    }
}

impl Boundable for Var {
    type Value = IntCst;

    fn leq(&self, ub: Self::Value) -> Lit {
        (*self).leq(ub)
    }

    fn geq(&self, lb: Self::Value) -> Lit {
        (*self).geq(lb)
    }
}

impl std::ops::Neg for Var {
    type Output = SignedVar;

    fn neg(self) -> Self::Output {
        SignedVar::minus(self)
    }
}
impl std::ops::Neg for &Var {
    type Output = SignedVar;

    fn neg(self) -> Self::Output {
        SignedVar::minus(*self)
    }
}
