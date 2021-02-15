use crate::lang::IntCst;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BoundValue(i32);

impl BoundValue {
    #[inline]
    pub fn new_lb(val: IntCst) -> Self {
        let res = BoundValue(-(val - 1));
        debug_assert_eq!(res.as_lb(), val);
        res
    }

    #[inline]
    pub fn as_lb(self) -> IntCst {
        -self.0 + 1
    }

    #[inline]
    pub fn new_ub(val: IntCst) -> Self {
        let res = BoundValue(val);
        debug_assert_eq!(res.as_ub(), val);
        res
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
