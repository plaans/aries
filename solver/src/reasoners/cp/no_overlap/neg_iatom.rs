use crate::{
    core::views::{Boundable, Term, VarView},
    prelude::*,
};

/// An [`IAtom`] or its negation. Used internally for the NoOverlap constraint to post the reverse view.
#[derive(Debug, Clone, Copy)]
pub(super) enum PMIAtom {
    Plus(IAtom),
    Minus(IAtom),
}

impl VarView for PMIAtom {
    type Value = IntCst;

    fn upper_bound(&self, dom: impl crate::core::views::Dom) -> Self::Value {
        match self {
            PMIAtom::Plus(iatom) => iatom.upper_bound(dom),
            PMIAtom::Minus(iatom) => -iatom.lower_bound(dom),
        }
    }

    fn lower_bound(&self, dom: impl crate::core::views::Dom) -> Self::Value {
        match self {
            PMIAtom::Plus(iatom) => iatom.lower_bound(dom),
            PMIAtom::Minus(iatom) => -iatom.upper_bound(dom),
        }
    }
}

impl Term for PMIAtom {
    fn variable(self) -> VarRef {
        match self {
            PMIAtom::Plus(iatom) | PMIAtom::Minus(iatom) => iatom.variable(),
        }
    }
}

impl Boundable for PMIAtom {
    type Value = IntCst;

    fn leq(&self, ub: Self::Value) -> Lit {
        match self {
            PMIAtom::Plus(iatom) => iatom.leq(ub),
            PMIAtom::Minus(iatom) => iatom.geq(-ub),
        }
    }

    fn geq(&self, lb: Self::Value) -> Lit {
        match self {
            PMIAtom::Plus(iatom) => iatom.geq(lb),
            PMIAtom::Minus(iatom) => iatom.leq(-lb),
        }
    }
}
