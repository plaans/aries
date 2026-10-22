use std::{
    fmt::{Debug, Display},
    ops::{Add, Mul, Neg, Sub},
    str::FromStr,
};

use num_rational::Rational64;

/// A real-numbered value, internally represented as a rational.
///
/// It mostly exist to hide implementation details from the underlying implementation (that may change in the future)
/// and customize the parsing and printing.
#[derive(Copy, Clone, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub struct RealValue(Rational64);

impl RealValue {
    pub const ZERO: RealValue = RealValue(Rational64::ZERO);

    pub fn new(numer: i64, denom: i64) -> Self {
        RealValue(Rational64::new(numer, denom))
    }

    pub fn from_integer(int: i64) -> Self {
        RealValue(Rational64::from_integer(int))
    }
    pub fn is_integer(&self) -> bool {
        self.0.is_integer()
    }

    pub fn numer(&self) -> &i64 {
        self.0.numer()
    }
    pub fn denom(&self) -> &i64 {
        self.0.denom()
    }
}
impl Display for RealValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0) // TODO: prefer decimal notation when it can be exact
    }
}
impl Debug for RealValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl Neg for RealValue {
    type Output = RealValue;

    fn neg(self) -> Self::Output {
        RealValue(-self.0)
    }
}

impl Sub for RealValue {
    type Output = RealValue;

    fn sub(self, rhs: Self) -> Self::Output {
        RealValue(self.0 - rhs.0)
    }
}

impl Add for RealValue {
    type Output = RealValue;

    fn add(self, rhs: Self) -> Self::Output {
        RealValue(self.0 + rhs.0)
    }
}

impl Mul for RealValue {
    type Output = RealValue;

    fn mul(self, rhs: Self) -> Self::Output {
        RealValue(self.0 * rhs.0)
    }
}
impl Mul<i64> for RealValue {
    type Output = RealValue;

    fn mul(self, rhs: i64) -> Self::Output {
        RealValue(self.0 * rhs)
    }
}

impl From<i64> for RealValue {
    fn from(value: i64) -> Self {
        RealValue(value.into())
    }
}

impl FromStr for RealValue {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: this is a very practical implementation but brings quite a few dependencies just for this function
        num_rational_parse::RationalParse::from_str_flex(s)
            .map(RealValue)
            .map_err(|e| e.to_string())
    }
}
