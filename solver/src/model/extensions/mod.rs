//! This module contains extension traits to [Model](crate::model::Model) and [Domains] that
//! when imported provide convenience methods.
//!
//! - [DisjunctionExt] allows querying the value of a disjunction,
//!   whether it is currently unit, ...
//! - [AssignmentExt] provides methods to query the value of expressions.

mod disjunction;
mod domains_ext;
mod format;
pub mod partial_assignment;

pub use disjunction::*;
pub use domains_ext::*;
pub use format::*;
