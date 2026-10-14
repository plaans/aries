//! Convenient re-exports of the types that are typically needed to build and solve
//! linear programs with [`Lp`].
//!
//! Import everything at once with:
//!
//! ```
//! use aries_lp::prelude::*;
//! ```

pub use crate::{
    Bound, ComparisonOp, Error, Lp, LinearExpr, OptimizationDirection, Solution, Variable,
    results::{FeasibilityStatus, InfeasibilityReason, SolveStatus, SolverFailure},
};
