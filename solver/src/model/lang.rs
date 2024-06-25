mod atom;
mod boolean;
mod cst;
pub mod expr;
mod fixed;
mod int;
pub mod linear;
pub mod reification;
mod sym;
mod validity_scope;
mod variables;

pub use atom::Atom;
pub use boolean::BVar;
pub use cst::Cst;
pub use fixed::{FAtom, FVar};
pub use int::{IAtom, IVar};
pub use validity_scope::*;

use crate::core::{IntCst, INT_CST_MAX, INT_CST_MIN};
use crate::model::types::TypeId;
pub use sym::{SAtom, SVar};
pub use variables::Variable;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub enum Type {
    Sym(TypeId),
    Int {
        lb: IntCst,
        ub: IntCst,
    },
    /// A fixed-point numeral, parameterized with its denominator.
    Fixed(IntCst),
    Bool,
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        match self {
            Type::Sym(_) | Type::Bool => false,
            Type::Int { .. } | Type::Fixed(_) => true,
        }
    }
}

impl From<Type> for Kind {
    fn from(tpe: Type) -> Self {
        match tpe {
            Type::Sym(_) => Kind::Sym,
            Type::Int { .. } => Kind::Int,
            Type::Fixed(denum) => Kind::Fixed(denum),
            Type::Bool => Kind::Bool,
        }
    }
}

impl Type {
    pub const UNBOUNDED_INT: Type = Type::Int {
        lb: INT_CST_MIN,
        ub: INT_CST_MAX,
    };
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub enum Kind {
    Bool,
    Int,
    /// A fixed-point numeral, parameterized with its denominator.
    Fixed(IntCst),
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

/// Given three types A, B and C with the following traits:
/// - From<B> for A, From<C> for B,
/// The marco implements the traits:
///  - From<C> for A
#[macro_export]
macro_rules! transitive_conversion {
    ($A: ty, $B: ty, $C: ty) => {
        impl From<$C> for $A {
            fn from(i: $C) -> Self {
                <$B>::from(i).into()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    type Model = crate::model::Model<&'static str>;

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
