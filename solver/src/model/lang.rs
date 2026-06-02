pub mod alternative;
mod bool_expr;
mod boolean;
pub mod element;
pub mod exclusive_choice;
pub mod expr;
mod fixed;
mod int;
mod int_expr;
pub mod linear; // TODO: make pub(crate)
pub mod max;
pub mod mul;
pub mod reification;
mod store;
mod sym;
mod validity_scope;
mod variables;

pub use crate::core::Lit;
pub use bool_expr::BoolExpr;
pub use boolean::BVar;
#[doc(hidden)]
pub use fixed::{FAtom, FVar, Rational};
pub use int::{IAtom, IVar};
pub use int_expr::IntExpr;
pub use linear::{LinearLeq, LinearSum, LinearTerm};
pub use store::{ModelWrapper, Store};
#[doc(hidden)]
pub use sym::{SAtom, SVar};
pub use validity_scope::*;
pub use variables::Variable;

use crate::core::{INT_CST_MAX, INT_CST_MIN, IntCst};
use crate::model::types::TypeId;

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

#[doc(hidden)]
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
/// - `From<B>` for `A`, `From<C>` for `B`,
/// - `TryFrom<A>` for `B`, `TryFrom<B>` for `C`
///
/// The macro implements the traits:
///  - `From<C>` for `A`
///  - `TryFrom<A>` for `C`
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
/// - `From<B>` for `A`, `From<C>` for `B`,
///
/// The macro implements the traits:
///  - `From<C>` for `A`
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
