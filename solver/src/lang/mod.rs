//! Types for representing expressions in the model.
//!
//!
//! ## Numeric types
//!
//! This module provides a hierarchy of numeric types from most specific to most general.
//! Each type can be converted into more general types through the `From` trait.
//!
//! | Type | Form | Represents | Implements |
//! |------|:----:|------------|------------|
//! | **[`IntCst`]** | `c` | Integer constant |  |
//! | **[`Var`]** | `X` | Single variable | [`VarView`], [`Term`], [`Boundable`], |
//! | [`SignedVar`] | `±X` | Signed variable | [`VarView`], [`Term`], [`Boundable`], |
//! | [`ScaledVar`] | `n·X` | Variable multiplied by constant | [`VarView`], [`Term`], [`Boundable`], |
//! | [`IAtom`] | `X + c` | Variable plus constant offset | [`VarView`], [`Term`], [`Boundable`], |
//! | [`LinTerm`] | `n·X + c` | Scaled variable plus constant | [`VarView`], [`Term`], [`Boundable`] |
//! | **[`LinSum`]** | `Σ(nᵢ·Xᵢ) + c` | Sum of scaled variables plus constant | [`IntExpr`] |
//! | `dyn` [`IntExpr`] | `unknown` | Arbitrary int expression |  |
//!
//! Most user code will only deal with [`Var`] (a single decision variable) and [`LinSum`] but one may face
//! other variants when doing arithmetic (but type inference is usually sufficient to ignore them).
//!
//! ```
//! # use aries::prelude::*;
//! # use aries::lang::linear::*;
//! let mut domains = Domains::new();
//! let x: Var = domains.new_var(0, 10);
//! let x: ScaledVar = 3 * x;
//! let x: LinTerm = x + 2;
//! let x: LinSum = x + domains.new_var(0,4);
//! ```
//!
//! ### Conversions
//!
//! ```text
//!        Var → SignedVar → ScaledVar → LinTerm → LinSum
//!          ↓                                  ↑
//! IntCst  →└────────────→ IAtom ──────────────┘
//! ```
//!
//! All conversions upward in the hierarchy are provided through `From` implementations,
//! while conversions downward are fallible and provided through `TryFrom` where applicable.
//!
//!
//! ## Boolean types
//!
//! Unlike, most constraint programming libraries, aries comes with a core boolean type [`Lit`]
//! that represents an upper or lower bound on a variables (`X <= c` or X >= c`).
//!
//! The [`Lit`] type is fundamental in the solver, representing decisions in the solvers,
//! elements of a [`Disjunction`], elements of learned clauses, ...
//!
//! While a `Lit` is fundamentally a statement on a [`Var`] it can be built for any type that implements [`Boundable`].
//!
//!
//!

#[cfg(doc)]
use crate::core::views::*;
#[cfg(doc)]
use crate::prelude::*;
#[cfg(doc)]
use linear::*;

pub mod alternative;
mod bool_expr;
mod boolean;
pub mod element;
pub mod exclusive_choice;
pub mod expr;
mod int;
mod int_expr;
pub mod linear;
pub mod max;
pub mod mul;
mod ops;
pub mod reification;
mod store;
mod validity_scope;

pub use crate::core::Lit;
pub use crate::core::Var;
pub use bool_expr::BoolExpr;
pub use boolean::BVar;
pub use int::IAtom;
pub use int_expr::IntExpr;
pub use store::{ModelWrapper, Store};
pub use validity_scope::*;

use crate::core::{INT_CST_MAX, INT_CST_MIN, IntCst};

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
    /// The expression is more general than the targetted one.
    MoreGeneral,
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
            ConversionError::MoreGeneral => write!(f, "more general than target"),
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
