use crate::core::{IntCst, Lit, SignedVar, VarRef};

pub trait Dom {
    fn upper_bound(&self, svar: SignedVar) -> IntCst;

    fn presence(&self, var: VarRef) -> Lit;

    fn lower_bound(&self, svar: SignedVar) -> IntCst {
        -self.upper_bound(-svar)
    }
}

impl<X> Dom for &X
where
    X: Dom,
{
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        (*self).upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        (*self).presence(var)
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

impl VarView for Lit {
    type Value = bool;

    fn upper_bound(&self, dom: impl Dom) -> Self::Value {
        // the upper bound is `false` if the Lit is not satisfiable anymore
        // i.e.,  given the lit `(var <= val)`  it is incompatible with the lower bound var >= val +1
        self.svar().lower_bound(dom) <= self.ub_value()
    }

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
        (*self).leq(ub)
    }

    #[inline]
    fn geq(&self, lb: Self::Value) -> Lit {
        (*self).geq(lb)
    }
}
