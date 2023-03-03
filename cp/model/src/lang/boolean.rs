use crate::lang::IVar;
use aries::core::*;

/// A boolean variable.
/// It is a wrapper around an (untyped) discrete variable to provide type safety.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct BVar(VarRef);

impl BVar {
    pub fn new(dvar: VarRef) -> Self {
        BVar(dvar)
    }

    /// Provides an integer view of this boolean variable
    /// where true <-> 1   and  false <-> 0
    pub fn int_view(self) -> IVar {
        IVar::new(self.0)
    }

    pub fn true_lit(self) -> Lit {
        Lit::geq(self, 1)
    }
    pub fn false_lit(self) -> Lit {
        Lit::leq(self, 0)
    }
}

impl From<BVar> for VarRef {
    fn from(i: BVar) -> Self {
        i.0
    }
}

impl From<BVar> for Lit {
    fn from(v: BVar) -> Self {
        v.true_lit()
    }
}

impl From<usize> for BVar {
    fn from(i: usize) -> Self {
        BVar(VarRef::from(i))
    }
}

impl From<BVar> for usize {
    fn from(b: BVar) -> Self {
        usize::from(b.0)
    }
}

impl From<BVar> for IVar {
    fn from(b: BVar) -> Self {
        IVar::new(b.0)
    }
}

impl std::ops::Not for BVar {
    type Output = Lit;

    fn not(self) -> Self::Output {
        self.false_lit()
    }
}
