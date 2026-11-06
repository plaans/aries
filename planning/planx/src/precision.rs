use num_integer::lcm;

use crate::RealValue;

/// Denotes the reachable values of a numeric expression in rational form.
#[derive(Debug, Copy, Clone)]
pub struct NumericPrecision {
    /// Least common multiplier for the numerator (or None if we have no idea)
    numer_lcm: Option<u64>,
    /// Least common multiplier for the denominator
    pub(crate) denom_lcm: u64,
}
impl NumericPrecision {
    /// Precision required to represent integer number.
    pub const INTEGER: NumericPrecision = NumericPrecision {
        numer_lcm: None,
        denom_lcm: 1,
    };

    /// Precision required to represent the given constant.
    pub fn from_constant(value: RealValue) -> Self {
        Self {
            numer_lcm: Some(value.numer().unsigned_abs()),
            denom_lcm: value.denom().unsigned_abs(),
        }
    }

    /// Updates the current precision to ensure it can also represent the other one.
    pub fn union(&mut self, other: NumericPrecision) {
        self.numer_lcm = match (self.numer_lcm, other.numer_lcm) {
            (Some(x), Some(y)) if x.next_power_of_two() + y.next_power_of_two() < 20 => Some(lcm(x, y)),
            _ => None,
        };
        self.denom_lcm = lcm(self.denom_lcm, other.denom_lcm);
    }
    pub fn combine_additive(&mut self, other: NumericPrecision) {
        // we give up finding out what are the possible values there
        self.numer_lcm = None;
        self.denom_lcm = lcm(self.denom_lcm, other.denom_lcm);
    }
    pub fn combine_mul(&mut self, other: NumericPrecision) {
        self.numer_lcm = match (self.numer_lcm, other.numer_lcm) {
            (Some(x), Some(y)) if x.next_power_of_two() + y.next_power_of_two() < 20 => Some(x * y),
            _ => None,
        };
        self.denom_lcm *= other.denom_lcm;
    }
    pub(crate) fn invert(&self) -> Option<Self> {
        self.numer_lcm.map(|self_numer_lcm| Self {
            numer_lcm: Some(self.denom_lcm),
            denom_lcm: self_numer_lcm,
        })
    }
}
impl Default for NumericPrecision {
    fn default() -> Self {
        Self {
            numer_lcm: Some(1),
            denom_lcm: 1,
        }
    }
}
impl From<RealValue> for NumericPrecision {
    fn from(value: RealValue) -> Self {
        Self::from_constant(value)
    }
}
