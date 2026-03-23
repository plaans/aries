use crate::core::*;

/// A positive or negative view of an integer variable.
/// The type has dense integer values and can be used as an index in an array.
///
/// It is represented on 32 bits where:
///  - the 31 most significant bits represent the variable
///  - the least significant bit represents the sign either positive (1) or negative (0).
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct SignedVar(u32);

impl SignedVar {
    #[inline]
    pub const fn from_raw(id: u32) -> Self {
        SignedVar(id)
    }

    pub const fn to_u32(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn plus(v: VarRef) -> Self {
        SignedVar((v.to_u32() << 1) + 1)
    }

    #[inline]
    pub const fn minus(v: VarRef) -> Self {
        SignedVar(v.to_u32() << 1)
    }

    /// Return the opposite view of the same variable.
    ///
    /// ```
    /// use aries::core::*;
    /// let var = VarRef::from(1u32);
    /// let plus_var = SignedVar::minus(var);
    /// let minus_var = SignedVar::plus(var);
    /// assert_eq!(plus_var.neg(), minus_var);
    /// assert_eq!(minus_var.neg(), plus_var);
    /// ```
    #[inline]
    pub const fn neg(self) -> Self {
        SignedVar(self.0 ^ 0x1)
    }

    #[inline]
    pub const fn is_minus(self) -> bool {
        (self.0 & 0x1) == 0
    }

    #[inline]
    pub const fn is_plus(self) -> bool {
        (self.0 & 0x1) == 1
    }

    #[inline]
    pub fn variable(self) -> VarRef {
        VarRef::from(self.0 >> 1)
    }

    /// Returns `1` if the variable is a positive view and `-1` otherwise
    #[inline]
    pub fn sign(self) -> IntCst {
        if self.is_plus() { 1 } else { -1 }
    }

    #[inline]
    pub fn leq(self, ub: IntCst) -> Lit {
        Lit::leq(self, ub)
    }
    #[inline]
    pub fn geq(self, lb: IntCst) -> Lit {
        Lit::geq(self, lb)
    }
}

impl std::ops::Neg for SignedVar {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self.neg()
    }
}

impl std::fmt::Debug for SignedVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_minus() {
            write!(f, "-")?;
        }
        write!(f, "{:?}", self.variable())
    }
}

impl From<SignedVar> for u32 {
    fn from(vb: SignedVar) -> Self {
        vb.to_u32()
    }
}

impl From<u32> for SignedVar {
    fn from(u: u32) -> Self {
        SignedVar::from_raw(u)
    }
}

impl From<SignedVar> for usize {
    fn from(vb: SignedVar) -> Self {
        vb.0 as usize
    }
}

impl From<usize> for SignedVar {
    fn from(u: usize) -> Self {
        SignedVar::from_raw(u as u32)
    }
}

impl From<VarRef> for SignedVar {
    fn from(value: VarRef) -> Self {
        SignedVar::plus(value)
    }
}
