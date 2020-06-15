use std::fmt::Display;
use std::ops::{Add, AddAssign, Neg, Sub};

/// TODO: this type is an aberration
pub trait FloatLike:
    Add<Self, Output = Self> + Display + Copy + Ord + Sub<Self, Output = Self> + Neg<Output = Self> + AddAssign<Self>
{
    fn zero() -> Self;
    fn infty() -> Self;
    fn neg_infty() -> Self;
    fn epsilon() -> Self;
}

impl FloatLike for i32 {
    fn zero() -> Self {
        0
    }

    fn infty() -> Self {
        std::i32::MAX / 2
    }

    fn neg_infty() -> Self {
        std::i32::MIN / 2
    }

    fn epsilon() -> Self {
        1
    }
}
