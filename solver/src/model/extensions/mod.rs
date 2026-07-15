//! This module contains extension traits to [Model] and [Domains] that
//! when imported provide convenience methods.
//!
//! - [DisjunctionExt] allows querying the value of a disjunction,
//!   whether it is currently unit, ...

#[cfg(doc)]
use crate::prelude::*;

mod disjunction;

pub use disjunction::*;
