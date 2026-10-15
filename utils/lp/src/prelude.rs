//! Convenient re-exports of the types that are typically needed to build and solve
//! linear programs with [`Lp`].
//!
//! Import everything at once with:
//!
//! ```
//! use aries_lp::prelude::*;
//! ```

pub use crate::{
    Bound, ComparisonOp, Error, Lp, OptimizationDirection, Problem, Solution, Variable,
    expr::{Constraint, LinExpr},
    results::{FeasibilityStatus, InfeasibilityReason, SolveStatus, SolverFailure},
};

/// `<=` Comparison operator.
pub const LEQ: ComparisonOp = ComparisonOp::Le;

/// `>=` Comparison operator.
pub const GEQ: ComparisonOp = ComparisonOp::Ge;

/// `=` Comparison operator.
pub const EQ: ComparisonOp = ComparisonOp::Eq;
