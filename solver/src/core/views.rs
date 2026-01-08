use crate::core::{IntCst, Lit, SignedVar, VarRef};

pub trait Dom {
    fn upper_bound(&self, svar: SignedVar) -> IntCst;

    fn presence(&self, var: VarRef) -> Lit;

    fn lower_bound(&self, svar: SignedVar) -> IntCst {
        -self.upper_bound(-svar)
    }
}

pub trait VarView {
    type Value: Ord;

    fn upper_bound(&self, dom: impl Dom) -> Self::Value;
    fn lower_bound(&self, dom: impl Dom) -> Self::Value;
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

pub trait Boundable {
    type Value;
    fn leq(&self, ub: Self::Value) -> Lit;
    fn geq(&self, lb: Self::Value) -> Lit;
}

impl Boundable for SignedVar {
    type Value = IntCst;

    #[inline]
    fn leq(&self, ub: Self::Value) -> Lit {
        (*self).leq(ub)
    }

    #[inline]
    fn geq(&self, lb: Self::Value) -> Lit {
        (*self).geq(lb)
    }
}
