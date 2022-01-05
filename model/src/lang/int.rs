use crate::lang::linear::IAtomScaled;
use crate::lang::ConversionError;
use aries_core::*;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt::Debug;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct IVar(VarRef);

impl Debug for IVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl IVar {
    pub const ZERO: IVar = IVar(VarRef::ZERO);

    pub const fn new(dvar: VarRef) -> Self {
        IVar(dvar)
    }

    pub fn leq(self, i: IntCst) -> Lit {
        Lit::leq(self, i)
    }

    pub fn geq(self, i: IntCst) -> Lit {
        Lit::geq(self, i)
    }

    pub fn lt(self, i: IntCst) -> Lit {
        Lit::lt(self, i)
    }

    pub fn gt(self, i: IntCst) -> Lit {
        Lit::gt(self, i)
    }
}

impl From<IVar> for VarRef {
    fn from(i: IVar) -> Self {
        i.0
    }
}

/// An int-valued atom `(variable + constant)`
/// It can be used to represent a constant value by using [IVar::ZERO] as the variable.
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct IAtom {
    pub var: IVar,
    pub shift: IntCst,
}

// Implement Debug for IAtom
impl Debug for IAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} + {:?}", self.var, self.shift)
    }
}

impl IAtom {
    pub const ZERO: IAtom = IAtom {
        var: IVar::ZERO,
        shift: 0,
    };
    pub fn new(var: IVar, shift: IntCst) -> IAtom {
        IAtom { var, shift }
    }

    /// A total order between the names of the atoms, not on their expected values.
    pub fn lexical_cmp(&self, other: &IAtom) -> Ordering {
        self.var.cmp(&other.var).then(self.shift.cmp(&other.shift))
    }

    /// Returns a literal representing whether this atom is lesser than the given value.
    pub fn lt_lit(self, value: IntCst) -> Lit {
        let rhs = value - self.shift;
        if self.var != IVar::ZERO {
            VarRef::from(self.var).lt(rhs)
        } else if 0 < rhs {
            Lit::TRUE
        } else {
            Lit::FALSE
        }
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
impl From<VarRef> for IAtom {
    fn from(v: VarRef) -> Self {
        IAtom::new(IVar::new(v), 0)
    }
}
impl From<IntCst> for IAtom {
    fn from(i: i32) -> Self {
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

impl std::ops::Mul<IntCst> for IVar {
    type Output = IAtomScaled;

    fn mul(self, rhs: IntCst) -> Self::Output {
        IAtomScaled::new(rhs, self.into())
    }
}
