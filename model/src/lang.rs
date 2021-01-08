mod atom;
mod boolean;
mod expr;
mod int;
mod sym;
mod variables;

use std::convert::TryFrom;
use std::hash::Hash;

use aries_collections::create_ref_type;

pub type IntCst = i32;
pub static INT_CST_MAX: IntCst = i32::MAX / 2 - 1; // TODO: this is a work around to avoid overflow

create_ref_type!(VarRef);

pub use atom::Atom;
pub use boolean::{BAtom, BExpr, BVar};
pub use expr::{Expr, Fun};
pub use int::{IAtom, IVar};

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
        }
    }
}

impl std::error::Error for ConversionError {}

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
    use crate::Model;

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

        let x = m.leq(a + 1, 6);
        check(&m, x, "(<= (+ a 1) 6)");

        let x = m.eq(a - 3, b);
        check(&m, x, "(= (- a 3) b)");

        let x = m.implies(true, x);
        check(&m, x, "(or false (= (- a 3) b))")
    }
}
