mod atom;
mod boolean;
pub mod expr;
mod int;
pub mod normal_form;
pub mod reification;
mod sym;
mod variables;

use aries_collections::create_ref_type;
use std::convert::TryFrom;
use std::hash::Hash;

/// Type representing an integer constant.
pub type IntCst = i32;

/// Overflow tolerant min value for integer constants.
/// It is used as a default for the lower bound of integer variable domains
pub const INT_CST_MIN: IntCst = IntCst::MIN / 2 + 1;

/// Overflow tolerant max value for integer constants.
/// It is used as a default for the upper bound of integer variable domains
pub const INT_CST_MAX: IntCst = IntCst::MAX / 2 - 1;

create_ref_type!(VarRef);

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

pub use atom::Atom;
pub use boolean::BVar;
pub use int::{IAtom, IVar};

use crate::literals::Lit;
use crate::types::TypeId;
pub use sym::{SAtom, SVar};
pub use variables::Variable;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum Type {
    Sym(TypeId),
    Int,
    Bool,
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub enum Kind {
    Bool,
    Int,
    Sym,
}

#[derive(Debug)]
pub enum ConversionError {
    TypeError,
    NotConstant,
    NotVariable,
    NotLiteral,
    NotExpression,
    /// This conversion occurs when trying to convert an expression into a variable and that,
    /// there is a variable but its value is modified. For instance, this would
    /// occur when trying to convert the atoms representing `!v` or `v + 1` for some variable `v`.
    NotPure,
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversionError::TypeError => write!(f, "type error"),
            ConversionError::NotConstant => write!(f, "not a constant"),
            ConversionError::NotVariable => write!(f, "not a variable"),
            ConversionError::NotPure => write!(f, "not a pure"),
            ConversionError::NotExpression => write!(f, "not an expression"),
            ConversionError::NotLiteral => write!(f, "not a bound"),
        }
    }
}

impl std::error::Error for ConversionError {}

impl From<ConversionError> for String {
    fn from(e: ConversionError) -> Self {
        e.to_string()
    }
}

/// Given three types A, B and C with the following traits:
/// - From<B> for A, From<C> for B,
/// - TryFrom<A> for B, TryFrom<B> for C
/// The marco implements the traits:
///  - From<C> for A
///  - TryFrom<A> for C
#[macro_export]
macro_rules! transitive_conversions {
    ($A: ty, $B: ty, $C: ty) => {
        impl From<$C> for $A {
            fn from(i: $C) -> Self {
                <$B>::from(i).into()
            }
        }

        impl TryFrom<$A> for $C {
            type Error = ConversionError;

            fn try_from(value: $A) -> Result<Self, Self::Error> {
                match <$B>::try_from(value) {
                    Ok(x) => <$C>::try_from(x),
                    Err(x) => Err(x),
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    type Model = crate::Model<&'static str>;

    fn check(m: &Model, x: impl Into<Atom>, result: &str) {
        assert_eq!(m.fmt(x).to_string(), result);
    }

    #[test]
    #[ignore] // TODO: fix syntax printing
    fn test_syntax() {
        let mut m = Model::new();

        let a = m.new_ivar(0, 10, "a");
        check(&m, a, "a");

        let b = m.new_ivar(0, 10, "b");

        let x = b + 1;
        check(&m, x, "(+ b 1)");

        let x = b - 1;
        check(&m, x, "(- b 1)");

        let x = x + 1;
        check(&m, x, "b");

        // let x = m.leq(a + 1, 6);
        // check(&m, x, "(<= (+ a 1) 6)");
        //
        // let x = m.eq(a - 3, b);
        // check(&m, x, "(= (- a 3) b)");
        //
        // let x = m.implies(true, x);
        // check(&m, x, "(or false (= (- a 3) b))")
    }
}
