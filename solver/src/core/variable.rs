use crate::core::Lit;
use crate::create_ref_type;
use std::{fmt::Debug, hash::Hash};

/// Type representing an integer constant.
#[cfg(all(feature = "i32", not(feature = "i64"), not(feature = "i128")))]
pub type IntCst = i32;

/// Type representing an integer constant.
#[cfg(all(feature = "i64", not(feature = "i128")))]
pub type IntCst = i64;

/// Type used to store the result of operations on `IntCst` that may overflow
#[cfg(all(feature = "i32", not(feature = "i64"), not(feature = "i128")))]
pub type LongCst = i64;

/// Type used to store the result of operations on `IntCst` that may overflow
#[cfg(all(feature = "i64", not(feature = "i128")))]
pub type LongCst = i128;

/// Type used to store the result of operations on `IntCst` that may overflow
#[cfg(feature = "i128")]
pub type LongCst = i128;

/// Type used to store the result of operations on `IntCst` that may overflow
#[cfg(feature = "i128")]
pub type IntCst = i128;

/// Convert IntCst to LongCst
pub(crate) const fn cst_int_to_long(cst: IntCst) -> LongCst {
    cst as LongCst
}

/// Convert u32 to IntCst
pub const fn u32_to_cst(cst: u32) -> IntCst {
    cst as IntCst
}

/// Convert LongCst to IntCst
pub(crate) const fn cst_long_to_int(cst: LongCst) -> IntCst {
    cst as IntCst
}

/// Overflow tolerant max value for integer constants.
/// It is used as a default for the upper bound of integer variable domains
pub const INT_CST_MAX: IntCst = IntCst::MAX / 4 - 1;

/// Overflow tolerant min value for integer constants.
/// It is used as a default for the lower bound of integer variable domains
pub const INT_CST_MIN: IntCst = -INT_CST_MAX;

create_ref_type!(VarRef);

// Implement Debug for VarRef
// `?` represents a variable
impl Debug for VarRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "var{:?}", self.to_u32())
    }
}

impl VarRef {
    /// A reserved special variable that is always equal to 0. It corresponds to the first representable VarRef.
    ///
    /// For efficiency reasons, this special case is not treated separately from the other variables, and it is the responsibility
    /// of the producers of VarRef to ensure that they only emit this value for variables whose domain is `[0,0]`.
    pub const ZERO: VarRef = VarRef::from_u32(0);

    /// A reserved special variable that is always equal to 1. It corresponds to the second representable VarRef.
    ///
    /// For efficiency reasons, this special case is not treated separately from the other variables, and it is the responsibility
    /// of the producers of VarRef to ensure that they only emit this value for variables whose domain is `[1,1]`.
    pub const ONE: VarRef = VarRef::from_u32(1);

    pub fn leq(self, i: IntCst) -> Lit {
        Lit::leq(self, i)
    }
    pub fn lt(self, i: IntCst) -> Lit {
        Lit::lt(self, i)
    }
    pub fn geq(self, i: IntCst) -> Lit {
        Lit::geq(self, i)
    }
    pub fn gt(self, i: IntCst) -> Lit {
        Lit::gt(self, i)
    }
}
