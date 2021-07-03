use crate::bounds::{Bound, BoundValue, Relation};
use crate::lang::VarRef;

/// Represents the upped or the lower bound of a particular variable.
/// The type has dense integer values and can by used an index in an array.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VarBound(u32);

impl VarBound {
    #[inline]
    pub const fn from_raw(id: u32) -> Self {
        VarBound(id)
    }

    pub const fn to_u32(self) -> u32 {
        self.0
    }

    #[inline]
    pub fn affected_by_relation(v: VarRef, rel: Relation) -> Self {
        debug_assert_eq!(Relation::Gt as u32, 0);
        debug_assert_eq!(Relation::Leq as u32, 1);
        VarBound((u32::from(v) << 1) + (rel as u32))
    }

    #[inline]
    pub fn ub(v: VarRef) -> Self {
        VarBound((u32::from(v) << 1) + 1)
    }

    #[inline]
    pub fn lb(v: VarRef) -> Self {
        VarBound(u32::from(v) << 1)
    }

    #[inline]
    pub fn bind(self, value: BoundValue) -> Bound {
        Bound::from_parts(self, value)
    }

    /// Return the other bound on the same variable.
    ///
    /// If this represents a lower bound, it will return the associated upper bound
    /// and vice versa.
    ///
    /// ```
    /// use aries_model::bounds::VarBound;
    /// use aries_model::lang::VarRef;
    /// let var = VarRef::from(1u32);
    /// let var_lb = VarBound::lb(var);
    /// let var_ub = VarBound::ub(var);
    /// assert_eq!(var_lb.symmetric_bound(), var_ub);
    /// assert_eq!(var_ub.symmetric_bound(), var_lb);
    /// ```
    #[inline]
    pub fn symmetric_bound(self) -> Self {
        VarBound(self.0 ^ 0x1)
    }

    #[inline]
    pub fn is_lb(self) -> bool {
        (self.0 & 0x1) == 0
    }

    #[inline]
    pub fn is_ub(self) -> bool {
        (self.0 & 0x1) == 1
    }

    #[inline]
    pub fn variable(self) -> VarRef {
        VarRef::from(self.0 >> 1)
    }
}

impl std::fmt::Debug for VarBound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({:?})", if self.is_ub() { "UB" } else { "LB" }, self.variable())
    }
}

impl From<VarBound> for u32 {
    fn from(vb: VarBound) -> Self {
        vb.to_u32()
    }
}

impl From<u32> for VarBound {
    fn from(u: u32) -> Self {
        VarBound::from_raw(u as u32)
    }
}

impl From<VarBound> for usize {
    fn from(vb: VarBound) -> Self {
        vb.0 as usize
    }
}

impl From<usize> for VarBound {
    fn from(u: usize) -> Self {
        VarBound::from_raw(u as u32)
    }
}
