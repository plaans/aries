use crate::theories::csp::range_set::RangeSet;
use aries::model::lang::IntCst;

pub struct Bounds {
    lb: IntCst,
    ub: IntCst,
}

impl From<(IntCst, IntCst)> for Bounds {
    fn from((lb, ub): (i32, i32)) -> Self {
        Bounds { lb, ub }
    }
}

pub struct SignedLit {
    root: Bounds,
    lit: RangeSet,
}

impl SignedLit {
    pub fn new(root: impl Into<Bounds>) -> Self {
        SignedLit {
            root: root.into(),
            lit: RangeSet::universe(),
        }
    }
}
