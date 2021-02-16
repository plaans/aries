use crate::lang::IntCst;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BoundValue(i32);

impl BoundValue {
    #[inline]
    pub fn lb(val: IntCst) -> Self {
        let res = BoundValue(-(val - 1));
        debug_assert_eq!(res.as_lb(), val);
        res
    }

    #[inline]
    pub fn as_lb(self) -> IntCst {
        -self.0 + 1
    }

    #[inline]
    pub fn ub(val: IntCst) -> Self {
        let res = BoundValue(val);
        debug_assert_eq!(res.as_ub(), val);
        res
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
    pub fn compatible_with_symmetric(self, other: BoundValue) -> bool {
        self.0 + other.0 > 0
    }

    #[inline]
    pub fn as_ub(self) -> IntCst {
        self.0
    }

    #[inline]
    pub fn stronger(self, other: BoundValue) -> bool {
        self.0 <= other.0
    }

    #[inline]
    pub fn strictly_stronger(self, other: BoundValue) -> bool {
        self.0 < other.0
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
#[derive(Copy, Clone, Hash, Debug)]
pub struct BoundValueAdd(IntCst);

impl BoundValueAdd {
    pub fn on_lb(increment: IntCst) -> Self {
        BoundValueAdd(-increment)
    }

    pub fn on_ub(increment: IntCst) -> Self {
        BoundValueAdd(increment)
    }
}

#[cfg(test)]
mod test {
    use crate::bounds::BoundValue;

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
}
