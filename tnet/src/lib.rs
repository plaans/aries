#![allow(dead_code)]

pub mod stn;

use std::ops::{Add, Neg, Sub};

/// A numeric type that implements all needed operation for the STN algorithms.
/// This trait is just a collection of abilities (other traits) and is automatically derived.
pub trait Time:
    Add<Self, Output = Self> + Sub<Self, Output = Self> + Neg<Output = Self> + num_traits::Zero + Ord + Copy
{
}

impl<T: Add<Self, Output = Self> + Copy + Ord + Sub<Self, Output = Self> + Neg<Output = Self> + num_traits::Zero> Time
    for T
{
}
