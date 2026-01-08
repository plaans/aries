use crate::{
    core::{
        IntCst, Lit, SignedVar,
        views::{Boundable, VarView},
    },
    create_ref_type,
};
use std::{fmt::Debug, hash::Hash};

create_ref_type!(VarRef);

// Implement Debug for VarRef
// `?` represents a variable
impl Debug for VarRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "var{:?}", self.to_u32())
    }
}

impl VarRef {
    /// A reserved special variable that is always equal to 0. It corresponds to the first representable VarRef.
    ///
    /// For efficiency reasons, this special case is not treated separately from the other variables, and it is the responsibility
    /// of the producers of VarRef to ensure that they only emit this value for variables whose domain is `[0,0]`.
    pub const ZERO: VarRef = VarRef::from_u32(0);

    /// A reserved special variable that is always equal to 1. It corresponds to the second representable VarRef.
    ///
    /// For efficiency reasons, this special case is not treated separately from the other variables, and it is the responsibility
    /// of the producers of VarRef to ensure that they only emit this value for variables whose domain is `[1,1]`.
    pub const ONE: VarRef = VarRef::from_u32(1);

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

impl VarView for VarRef {
    type Value = IntCst;

    fn upper_bound(&self, dom: impl super::views::Dom) -> Self::Value {
        dom.upper_bound(SignedVar::plus(*self))
    }

    fn lower_bound(&self, dom: impl super::views::Dom) -> Self::Value {
        dom.lower_bound(SignedVar::plus(*self))
    }
}

impl Boundable for VarRef {
    type Value = IntCst;

    fn leq(&self, ub: Self::Value) -> Lit {
        (*self).leq(ub)
    }

    fn geq(&self, lb: Self::Value) -> Lit {
        (*self).geq(lb)
    }
}
