use crate::Lit;
use aries_collections::create_ref_type;
use std::{fmt::Debug, hash::Hash};

/// Type representing an integer constant.
pub type IntCst = i32;

/// Overflow tolerant min value for integer constants.
/// It is used as a default for the lower bound of integer variable domains
pub const INT_CST_MIN: IntCst = IntCst::MIN / 2 + 1;

/// Overflow tolerant max value for integer constants.
/// It is used as a default for the upper bound of integer variable domains
pub const INT_CST_MAX: IntCst = IntCst::MAX / 2 - 1;

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
