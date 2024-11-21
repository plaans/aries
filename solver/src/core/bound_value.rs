use num_traits::ConstZero;

use crate::core::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UpperBound(IntCst);

impl UpperBound {
    #[inline]
    pub const fn lb(val: IntCst) -> Self {
        UpperBound(-val)
    }

    #[inline]
    pub const fn ub(val: IntCst) -> Self {
        UpperBound(val)
    }

    /// Given two bound values where one represent a lower bound and the other
    /// represent an upper bound, returns true if the two are incompatible.
    ///
    /// ```
    /// use aries::core::*;
    /// assert!(!UpperBound::lb(5).compatible_with_symmetric(UpperBound::ub(4)));
    /// assert!(UpperBound::lb(5).compatible_with_symmetric(UpperBound::ub(5)));
    /// assert!(UpperBound::lb(5).compatible_with_symmetric(UpperBound::ub(6)));
    /// assert!(UpperBound::lb(-5).compatible_with_symmetric(UpperBound::ub(4)));
    /// assert!(UpperBound::lb(-5).compatible_with_symmetric(UpperBound::ub(5)));
    /// assert!(UpperBound::lb(-5).compatible_with_symmetric(UpperBound::ub(6)));
    /// // the order of the values does not matter:
    /// assert!(UpperBound::ub(5).compatible_with_symmetric(UpperBound::lb(4)));
    /// assert!(UpperBound::ub(5).compatible_with_symmetric(UpperBound::lb(5)));
    /// assert!(!UpperBound::ub(5).compatible_with_symmetric(UpperBound::lb(6)));
    /// ```
    #[inline]
    pub const fn compatible_with_symmetric(self, other: UpperBound) -> bool {
        cst_to_acc(self.0) + cst_to_acc(other.0) >= ConstZero::ZERO
    }

    /// Return true if the two bound represent a singleton domain.
    /// This should be called with a lower and an upper bound.
    ///
    /// ```
    /// use aries::core::UpperBound;
    /// assert!(!UpperBound::lb(5).equal_to_symmetric(UpperBound::ub(4)));
    /// assert!(UpperBound::lb(5).equal_to_symmetric(UpperBound::ub(5)));
    /// assert!(!UpperBound::lb(5).equal_to_symmetric(UpperBound::ub(6)));
    /// // the order of the values does not matter:
    /// assert!(!UpperBound::ub(5).equal_to_symmetric(UpperBound::lb(4)));
    /// assert!(UpperBound::ub(5).equal_to_symmetric(UpperBound::lb(5)));
    /// assert!(!UpperBound::ub(5).equal_to_symmetric(UpperBound::lb(6)));
    /// ```
    #[inline]
    pub const fn equal_to_symmetric(self, other: UpperBound) -> bool {
        self.0 + other.0 == 0
    }

    #[inline]
    pub const fn as_int(self) -> IntCst {
        self.0
    }

    #[inline]
    pub const fn stronger(self, other: UpperBound) -> bool {
        self.0 <= other.0
    }

    #[inline]
    pub const fn strictly_stronger(self, other: UpperBound) -> bool {
        self.0 < other.0
    }
}

impl std::ops::Sub<UpperBound> for UpperBound {
    type Output = BoundValueAdd;

    fn sub(self, rhs: UpperBound) -> Self::Output {
        BoundValueAdd(self.0 - rhs.0)
    }
}

impl std::ops::Add<BoundValueAdd> for UpperBound {
    type Output = UpperBound;

    #[inline]
    fn add(self, rhs: BoundValueAdd) -> Self::Output {
        UpperBound(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign<BoundValueAdd> for UpperBound {
    #[inline]
    fn add_assign(&mut self, rhs: BoundValueAdd) {
        *self = *self + rhs
    }
}

impl std::ops::Sub<BoundValueAdd> for UpperBound {
    type Output = UpperBound;

    #[inline]
    fn sub(self, rhs: BoundValueAdd) -> Self::Output {
        UpperBound(self.0 - rhs.0)
    }
}

/// Represents an addition to an upper or lower bound that can be applied to a [BoundValue] .
/// This is a wrapper around a signed integer, to make sure the representation is compatible
/// with the one of the bound value.
///
/// ```
/// use aries::core::*;
/// let ub_add = BoundValueAdd::on_ub(5);
/// let lb_add = BoundValueAdd::on_ub(-4);
/// assert_eq!(UpperBound::ub(3) + BoundValueAdd::on_ub(5), UpperBound::ub(8));
/// assert_eq!(UpperBound::ub(-3) + BoundValueAdd::on_ub(5), UpperBound::ub(2));
/// assert_eq!(UpperBound::ub(-3) + BoundValueAdd::on_ub(-5), UpperBound::ub(-8));
/// assert_eq!(UpperBound::lb(3) + BoundValueAdd::on_lb(5), UpperBound::lb(8));
/// assert_eq!(UpperBound::lb(-3) + BoundValueAdd::on_lb(5), UpperBound::lb(2));
/// assert_eq!(UpperBound::lb(-3) + BoundValueAdd::on_lb(-5), UpperBound::lb(-8));
/// ```
#[derive(Copy, Clone, Hash, Debug, Ord, PartialOrd, PartialEq, Eq)]
pub struct BoundValueAdd(IntCst);

impl BoundValueAdd {
    /// The zero value addition, independently of whether it represents applies on lower or upper literals.
    pub const ZERO: BoundValueAdd = BoundValueAdd(0);

    /// Adding this to a [BoundValue] is equivalent to relaxing it to the next value.
    pub const RELAXATION: BoundValueAdd = BoundValueAdd(1);

    /// Adding this to a [BoundValue] is equivalent to tightening it to the next value.
    pub const TIGHTENING: BoundValueAdd = BoundValueAdd(-1);

    /// The maximum representable value. Not that using it anything else than a default value for comparison
    /// is likely to result in an overflow.
    pub const MAX: BoundValueAdd = BoundValueAdd(INT_CST_MAX);

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

    /// Returns true if the addition of this value would result in a tighter bound than the addition of the other.
    pub fn is_tighter_than(self, other: Self) -> bool {
        self.0 < other.0
    }

    /// Returns the raw value of a
    pub fn raw_value(self) -> IntCst {
        self.0
    }

    /// Transforms a lb addition into an ub addition an vice versa
    pub fn reciprocal(self) -> BoundValueAdd {
        BoundValueAdd(-self.0)
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
    use crate::core::*;

    #[test]
    fn test_compatibility() {
        let n = 10;
        for lb in -n..n {
            let x = UpperBound::lb(lb);
            for ub in -n..lb {
                let y = UpperBound::ub(ub);
                assert!(!x.compatible_with_symmetric(y), "{}", "Compatible [{lb}, {ub}]");
                assert!(!y.compatible_with_symmetric(x), "{}", "Compatible [{lb}, {ub}]");
            }

            for ub in lb..n {
                let y = UpperBound::ub(ub);
                assert!(x.compatible_with_symmetric(y), "{}", "Incompatible [{lb}, {ub}]");
                assert!(y.compatible_with_symmetric(x), "{}", "Incompatible [{lb}, {ub}]");
            }
            println!();
        }
    }

    #[test]
    fn test_bound_value_additions() {
        let u1 = UpperBound::ub(1);
        let u5 = UpperBound::ub(5);
        let l1 = UpperBound::lb(1);
        let l5 = UpperBound::lb(5);

        assert_eq!(u1 - u5, BoundValueAdd::on_ub(-4));
        assert_eq!(u5 - u1, BoundValueAdd::on_ub(4));

        assert_eq!(l1 - l5, BoundValueAdd::on_lb(-4));
        assert_eq!(l5 - l1, BoundValueAdd::on_lb(4));

        fn t(b1: UpperBound, b2: UpperBound) {
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
