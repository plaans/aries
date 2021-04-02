use crate::lang::IntCst;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BoundValue(i32);

impl BoundValue {
    #[inline]
    pub const fn lb(val: IntCst) -> Self {
        BoundValue(-(val - 1))
    }

    #[inline]
    pub const fn as_lb(self) -> IntCst {
        -self.0 + 1
    }

    #[inline]
    pub const fn ub(val: IntCst) -> Self {
        BoundValue(val)
    }

    /// Given two bound values where one represent a lower bound and the other
    /// represent an upper bound, returns true if the two are incompatible.
    ///
    /// ```
    /// use aries_model::bounds::BoundValue;
    /// assert!(!BoundValue::lb(5).compatible_with_symmetric(BoundValue::ub(4)));
    /// assert!(BoundValue::lb(5).compatible_with_symmetric(BoundValue::ub(5)));
    /// assert!(BoundValue::lb(5).compatible_with_symmetric(BoundValue::ub(6)));
    /// assert!(BoundValue::lb(-5).compatible_with_symmetric(BoundValue::ub(4)));
    /// assert!(BoundValue::lb(-5).compatible_with_symmetric(BoundValue::ub(5)));
    /// assert!(BoundValue::lb(-5).compatible_with_symmetric(BoundValue::ub(6)));
    /// // the order of the values does not matter:
    /// assert!(BoundValue::ub(5).compatible_with_symmetric(BoundValue::lb(4)));
    /// assert!(BoundValue::ub(5).compatible_with_symmetric(BoundValue::lb(5)));
    /// assert!(!BoundValue::ub(5).compatible_with_symmetric(BoundValue::lb(6)));
    /// ```
    #[inline]
    pub const fn compatible_with_symmetric(self, other: BoundValue) -> bool {
        self.0 + other.0 > 0
    }

    /// Return true if the two bound represent a singleton domain.
    /// This should be called with a lower and an upper bound.
    ///
    /// ```
    /// use aries_model::bounds::BoundValue;
    /// assert!(!BoundValue::lb(5).equal_to_symmetric(BoundValue::ub(4)));
    /// assert!(BoundValue::lb(5).equal_to_symmetric(BoundValue::ub(5)));
    /// assert!(!BoundValue::lb(5).equal_to_symmetric(BoundValue::ub(6)));
    /// // the order of the values does not matter:
    /// assert!(!BoundValue::ub(5).equal_to_symmetric(BoundValue::lb(4)));
    /// assert!(BoundValue::ub(5).equal_to_symmetric(BoundValue::lb(5)));
    /// assert!(!BoundValue::ub(5).equal_to_symmetric(BoundValue::lb(6)));
    /// ```
    #[inline]
    pub const fn equal_to_symmetric(self, other: BoundValue) -> bool {
        self.0 + other.0 == 1
    }

    #[inline]
    pub const fn as_ub(self) -> IntCst {
        self.0
    }

    #[inline]
    pub const fn stronger(self, other: BoundValue) -> bool {
        self.0 <= other.0
    }

    #[inline]
    pub const fn strictly_stronger(self, other: BoundValue) -> bool {
        self.0 < other.0
    }

    #[inline]
    pub const fn neg(self) -> Self {
        BoundValue(-self.0)
    }
}

impl std::ops::Sub<BoundValue> for BoundValue {
    type Output = BoundValueAdd;

    fn sub(self, rhs: BoundValue) -> Self::Output {
        BoundValueAdd(self.0 - rhs.0)
    }
}

impl std::ops::Neg for BoundValue {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        self.neg()
    }
}

impl std::ops::Add<BoundValueAdd> for BoundValue {
    type Output = BoundValue;

    #[inline]
    fn add(self, rhs: BoundValueAdd) -> Self::Output {
        BoundValue(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign<BoundValueAdd> for BoundValue {
    #[inline]
    fn add_assign(&mut self, rhs: BoundValueAdd) {
        *self = *self + rhs
    }
}

impl std::ops::Sub<BoundValueAdd> for BoundValue {
    type Output = BoundValue;

    #[inline]
    fn sub(self, rhs: BoundValueAdd) -> Self::Output {
        BoundValue(self.0 - rhs.0)
    }
}

/// Represents an addition to an upper or lower bound that can be applied to a [BoundValue] .
/// This is a wrapper around a signed integer, to make sure the representation is compatible
/// with the one of the bound value.
///
/// ```
/// use aries_model::bounds::{BoundValue, BoundValueAdd};
/// let ub_add = BoundValueAdd::on_ub(5);
/// let lb_add = BoundValueAdd::on_ub(-4);
/// assert_eq!(BoundValue::ub(3) + BoundValueAdd::on_ub(5), BoundValue::ub(8));
/// assert_eq!(BoundValue::ub(-3) + BoundValueAdd::on_ub(5), BoundValue::ub(2));
/// assert_eq!(BoundValue::ub(-3) + BoundValueAdd::on_ub(-5), BoundValue::ub(-8));
/// assert_eq!(BoundValue::lb(3) + BoundValueAdd::on_lb(5), BoundValue::lb(8));
/// assert_eq!(BoundValue::lb(-3) + BoundValueAdd::on_lb(5), BoundValue::lb(2));
/// assert_eq!(BoundValue::lb(-3) + BoundValueAdd::on_lb(-5), BoundValue::lb(-8));
/// ```
#[derive(Copy, Clone, Hash, Debug, Ord, PartialOrd, PartialEq, Eq)]
pub struct BoundValueAdd(IntCst);

impl BoundValueAdd {
    /// Construct the BVA that represents an update on a lower bound
    pub fn on_lb(increment: IntCst) -> Self {
        BoundValueAdd(-increment)
    }

    /// Returns the value used to build this BVA, with the assumption that
    /// it was buit as a lower bound increment
    pub fn as_lb_add(self) -> IntCst {
        -self.0
    }

    pub fn on_ub(increment: IntCst) -> Self {
        BoundValueAdd(increment)
    }

    pub fn as_ub_add(self) -> IntCst {
        self.0
    }

    /// Returns true if adding this value to a bound will make it tighter
    pub fn is_tightening(self) -> bool {
        self.0 < 0
    }

    /// Returns the raw value of a
    pub fn raw_value(self) -> IntCst {
        self.0
    }
}

impl std::ops::Add<BoundValueAdd> for BoundValueAdd {
    type Output = BoundValueAdd;

    fn add(self, rhs: BoundValueAdd) -> Self::Output {
        BoundValueAdd(self.0 + rhs.0)
    }
}

#[cfg(test)]
mod test {
    use crate::bounds::{BoundValue, BoundValueAdd};

    #[test]
    fn test_compatibility() {
        let n = 10;
        for lb in -n..n {
            let x = BoundValue::lb(lb);
            for ub in -n..lb {
                let y = BoundValue::ub(ub);
                assert!(!x.compatible_with_symmetric(y), "Compatible [{}, {}]", lb, ub);
                assert!(!y.compatible_with_symmetric(x), "Compatible [{}, {}]", lb, ub);
            }

            for ub in lb..n {
                let y = BoundValue::ub(ub);
                assert!(x.compatible_with_symmetric(y), "Incompatible [{}, {}]", lb, ub);
                assert!(y.compatible_with_symmetric(x), "Incompatible [{}, {}]", lb, ub);
            }
            println!();
        }
    }

    #[test]
    fn test_bound_value_additions() {
        let u1 = BoundValue::ub(1);
        let u5 = BoundValue::ub(5);
        let l1 = BoundValue::lb(1);
        let l5 = BoundValue::lb(5);

        assert_eq!(u1 - u5, BoundValueAdd::on_ub(-4));
        assert_eq!(u5 - u1, BoundValueAdd::on_ub(4));

        assert_eq!(l1 - l5, BoundValueAdd::on_lb(-4));
        assert_eq!(l5 - l1, BoundValueAdd::on_lb(4));

        fn t(b1: BoundValue, b2: BoundValue) {
            assert_eq!(b2, b1 - (b1 - b2));
            assert_eq!(b1, b2 - (b2 - b1))
        }

        t(u1, u5);
        t(u1, u1);
        t(u5, u1);

        t(l1, l5);
        t(l1, l1);
        t(l5, l1);
    }
}
