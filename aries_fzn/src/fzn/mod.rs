//! Flatzinc modelization.

pub mod constraint;
pub mod domain;
pub mod model;
pub mod output;
pub mod par;
pub mod parser;
pub mod solve;
pub mod types;
pub mod var;

mod fzn;
mod name;
mod parvar;

pub use fzn::Fzn;
pub use name::Name;
pub use parvar::ParVar;
