//! Module that re-export most commonly used types and traits to ease import.

pub use crate::core::literals::Conjunction;
pub use crate::core::literals::Disjunction;
pub use crate::core::state::Domains;
pub use crate::core::state::Solution;
pub use crate::core::views::Dom;
pub use crate::core::{IntCst, Lit, SignedVar, Var};
pub use crate::lang::IAtom;
pub use crate::lang::linear::{LinSum, LinTerm};
pub use crate::lang::{BoolExpr, IntExpr};
pub use crate::solver::SearchLimit;
pub use crate::solver::Solver;

pub use crate::lang::expr::*;

pub type Model = crate::model::Model<String>;

pub use crate::core::INT_CST_MAX;
pub use crate::core::INT_CST_MIN;

#[deprecated = "Use `Var` Instead"]
#[doc(hidden)]
#[allow(deprecated)]
pub use crate::core::VarRef;
