use std::ops::{Add, Neg, Sub};

/// A numeric type that implements all needed operation for the STN algorithms.
/// This trait is just a collection of abilities (other traits) and is automatically derived.
pub trait Time:
    Add<Self, Output = Self> + Sub<Self, Output = Self> + Neg<Output = Self> + num_traits::Zero + Ord + Copy + Step
{
}

impl<
        T: Add<Self, Output = Self> + Sub<Self, Output = Self> + Copy + Ord + Neg<Output = Self> + num_traits::Zero + Step,
    > Time for T
{
}

pub trait Step {
    fn step() -> Self;
}

impl Step for i32 {
    fn step() -> Self {
        1
    }
}

/// Saturating signed integer. This integer type will never overflow but instead
/// will saturate at its bounds.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct SaturatingI32(i32);

impl From<i32> for SaturatingI32 {
    fn from(i: i32) -> Self {
        SaturatingI32(i)
    }
}

impl Add<Self> for SaturatingI32 {
    type Output = Self;

    fn add(self, rhs: SaturatingI32) -> Self::Output {
        SaturatingI32(self.0.saturating_add(rhs.0))
    }
}

impl Neg for SaturatingI32 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        SaturatingI32(-self.0)
    }
}

impl num_traits::Zero for SaturatingI32 {
    fn zero() -> Self {
        0i32.into()
    }

    fn is_zero(&self) -> bool {
        self.0 == 0i32
    }
}
