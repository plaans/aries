//! Crate to make aries compatible with minizinc.
//!
//! Minizinc problems are compiled to flatzinc which is the
//! format supported by this crate.

// Disable clippy lint about module inception
#![allow(clippy::module_inception)]

pub mod aries;
pub mod cli;
pub mod fzn;
