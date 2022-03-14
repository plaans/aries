use crate::lang::{ConversionError, IAtom, IVar};
use aries_core::{IntCst, VarRef};
use std::cmp::Ordering;
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;

/// Represents a limited form of fixed-point number `num / denom` where
///  - the numerator is an int variable
///  - the denominator `denom` is a constant integer.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FVar {
    pub num: IVar,
    pub denom: IntCst,
}

impl FVar {
    pub fn new(num: IVar, denom: IntCst) -> FVar {
        assert_ne!(denom, 0);
        FVar { num, denom }
    }
}

impl Debug for FVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FVar({:?}/{:?})", self.num, self.denom)
    }
}

impl From<FVar> for VarRef {
    fn from(f: FVar) -> Self {
        f.num.into()
    }
}

impl std::ops::Add<Epsilon> for FVar {
    type Output = FAtom;

    fn add(self, _: Epsilon) -> Self::Output {
        FAtom::new(self.num + 1, self.denom)
    }
}

impl std::ops::Add<IntCst> for FVar {
    type Output = FAtom;

    fn add(self, i: IntCst) -> Self::Output {
        FAtom::new(self.num + i * self.denom, self.denom)
    }
}

/// Represents a limited form of fixed-point number `num / denom` where
///  - the numerator is an int atom (sum of an int variable and a constant)
///  - the denominator `denom` is a constant integer.
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct FAtom {
    pub num: IAtom,
    pub denom: IntCst,
}

//Implement Debug for FAtom
// `?` represents a variable
impl Debug for FAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "?f({:?})", self.num)
    }
}
/// The smallest increment of a fixed-point expression.
pub struct Epsilon;

impl FAtom {
    /// The smallest increment of a fixed-point expression.
    pub const EPSILON: Epsilon = Epsilon;

    pub fn new(num: IAtom, denom: IntCst) -> FAtom {
        assert_ne!(denom, 0);
        FAtom { num, denom }
    }
}

impl PartialOrd for FAtom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.denom == other.denom {
            self.num.partial_cmp(&other.num)
        } else {
            None
        }
    }
}

impl From<FVar> for FAtom {
    fn from(v: FVar) -> Self {
        FAtom::new(v.num.into(), v.denom)
    }
}

impl TryFrom<FAtom> for FVar {
    type Error = ConversionError;

    fn try_from(value: FAtom) -> Result<Self, Self::Error> {
        Ok(FVar::new(value.num.try_into()?, value.denom))
    }
}

impl std::ops::Add<Epsilon> for FAtom {
    type Output = FAtom;

    fn add(self, _: Epsilon) -> Self::Output {
        FAtom::new(self.num + 1, self.denom)
    }
}

impl std::ops::Add<IntCst> for FAtom {
    type Output = FAtom;

    fn add(self, i: IntCst) -> Self::Output {
        FAtom::new(self.num + i * self.denom, self.denom)
    }
}
