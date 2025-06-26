//! Module that re-export most commonly used types and traits to ease import.

pub use crate::core::state::Domains;
pub use crate::core::{IntCst, Lit, SignedVar, VarRef};
pub use crate::model::Model;
pub use crate::model::extensions::DomainsExt;
pub use crate::model::lang::{IAtom, IVar};
pub use crate::solver::SearchLimit;
pub use crate::solver::Solver;
