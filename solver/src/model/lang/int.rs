use crate::core::views::{Boundable, VarView};
use crate::core::*;
use crate::model::lang::ConversionError;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt::Debug;

pub type IVar = Var;

/// An int-valued atom `(variable + constant)`
/// It can be used to represent a constant value by using [IVar::ZERO] as the variable.
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct IAtom {
    pub var: IVar,
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
        if self.var == IVar::ZERO {
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
        var: IVar::ZERO,
        shift: 0,
    };
    pub const ONE: IAtom = IAtom {
        var: IVar::ZERO,
        shift: 1,
    };
    pub const TRUE: IAtom = Self::ONE;
    pub const FALSE: IAtom = Self::ZERO;
    pub fn new(var: IVar, shift: IntCst) -> IAtom {
        IAtom { var, shift }
    }

    /// Returns a literal representing whether this atom is lesser than the given value.
    pub fn lt_lit(self, value: IntCst) -> Lit {
        let rhs = value - self.shift;
        if self.var != IVar::ZERO {
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
        if self.var != IVar::ZERO {
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

impl From<IVar> for IAtom {
    fn from(v: IVar) -> Self {
        IAtom::new(v, 0)
    }
}

impl From<IntCst> for IAtom {
    fn from(i: IntCst) -> Self {
        IAtom::new(IVar::ZERO, i)
    }
}

impl TryFrom<IAtom> for IVar {
    type Error = ConversionError;

    fn try_from(value: IAtom) -> Result<Self, Self::Error> {
        if value.shift == 0 {
            Ok(value.var)
        } else {
            Err(ConversionError::NotPure)
        }
    }
}

impl TryFrom<IAtom> for IntCst {
    type Error = ConversionError;

    fn try_from(value: IAtom) -> Result<Self, Self::Error> {
        match value.var {
            IVar::ZERO => Ok(value.shift),
            _ => Err(ConversionError::NotConstant),
        }
    }
}

impl std::ops::Add<IntCst> for IAtom {
    type Output = IAtom;

    fn add(self, rhs: IntCst) -> Self::Output {
        IAtom::new(self.var, self.shift + rhs)
    }
}
impl std::ops::Add<IntCst> for IVar {
    type Output = IAtom;

    fn add(self, rhs: IntCst) -> Self::Output {
        IAtom::new(self, rhs)
    }
}
impl std::ops::Sub<IntCst> for IAtom {
    type Output = IAtom;

    fn sub(self, rhs: IntCst) -> Self::Output {
        IAtom::new(self.var, self.shift - rhs)
    }
}
impl std::ops::Sub<IntCst> for IVar {
    type Output = IAtom;

    fn sub(self, rhs: IntCst) -> Self::Output {
        IAtom::new(self, -rhs)
    }
}
impl std::ops::Add<usize> for IAtom {
    type Output = IAtom;

    fn add(self, rhs: usize) -> Self::Output {
        IAtom::new(self.var, self.shift + IntCst::try_from(rhs).unwrap())
    }
}
impl std::ops::Add<usize> for IVar {
    type Output = IAtom;

    fn add(self, rhs: usize) -> Self::Output {
        IAtom::new(self, IntCst::try_from(rhs).unwrap())
    }
}
impl std::ops::Sub<usize> for IAtom {
    type Output = IAtom;

    fn sub(self, rhs: usize) -> Self::Output {
        IAtom::new(self.var, self.shift - IntCst::try_from(rhs).unwrap())
    }
}
impl std::ops::Sub<usize> for IVar {
    type Output = IAtom;

    fn sub(self, rhs: usize) -> Self::Output {
        IAtom::new(self, -IntCst::try_from(rhs).unwrap())
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
