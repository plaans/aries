use crate::core::views::{Boundable, VarView};
use crate::core::*;
use std::cmp::Ordering;
use std::fmt::Debug;

/// An int-valued atom `(variable + constant)`
/// It can be used to represent a constant value by using [Var::ZERO] as the variable.
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct IAtom {
    pub var: Var,
    pub shift: IntCst,
}

impl VarView for IAtom {
    type Value = IntCst;

    fn upper_bound(&self, dom: impl views::Dom) -> Self::Value {
        self.var.upper_bound(dom) + self.shift
    }

    fn lower_bound(&self, dom: impl views::Dom) -> Self::Value {
        self.var.lower_bound(dom) + self.shift
    }
}

// Implement Debug for IAtom
impl Debug for IAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.var == Var::ZERO {
            write!(f, "{}", self.shift)
        } else if self.shift == 0 {
            write!(f, "{:?}", self.var)
        } else {
            write!(f, "{:?} + {:?}", self.var, self.shift)
        }
    }
}

impl IAtom {
    pub const ZERO: IAtom = IAtom {
        var: Var::ZERO,
        shift: 0,
    };
    pub const ONE: IAtom = IAtom {
        var: Var::ZERO,
        shift: 1,
    };
    pub const TRUE: IAtom = Self::ONE;
    pub const FALSE: IAtom = Self::ZERO;
    pub fn new(var: Var, shift: IntCst) -> IAtom {
        IAtom { var, shift }
    }

    /// Returns a literal representing whether this atom is lesser than the given value.
    pub fn lt_lit(self, value: IntCst) -> Lit {
        let rhs = value - self.shift;
        if self.var != Var::ZERO {
            self.var.lt(rhs)
        } else if 0 < rhs {
            Lit::TRUE
        } else {
            Lit::FALSE
        }
    }

    /// Returns a literal representing whether this atom is lesser than or equal to the given value.
    pub fn le_lit(self, value: IntCst) -> Lit {
        self.lt_lit(value + 1)
    }

    /// Returns a literal representing whether this atom is greater than the given value.
    pub fn gt_lit(self, value: IntCst) -> Lit {
        let rhs = value - self.shift;
        if self.var != Var::ZERO {
            self.var.gt(rhs)
        } else if 0 > rhs {
            Lit::TRUE
        } else {
            Lit::FALSE
        }
    }

    /// Returns a literal representing whether this atom is greater than or equal to the given value.
    pub fn ge_lit(self, value: IntCst) -> Lit {
        self.gt_lit(value - 1)
    }
}

/// Comparison on the values that can be taken for two atoms.
/// We can only carry out the comparison if they are on the same variable.
/// Otherwise, we cannot say in which order their values will be.
impl PartialOrd for IAtom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.var == other.var {
            Some(self.shift.cmp(&other.shift))
        } else {
            None
        }
    }
}

impl Boundable for IAtom {
    type Value = IntCst;

    #[inline]
    fn leq(&self, ub: Self::Value) -> Lit {
        // var + shift <= ub <=> var <= ub - shib
        self.var.leq(ub - self.shift)
    }

    #[inline]
    fn geq(&self, lb: Self::Value) -> Lit {
        self.var.geq(lb - self.shift)
    }
}
