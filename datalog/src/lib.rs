#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

pub(crate) mod merge;
mod program;
mod rules;
mod tables;

pub use crate::program::*;
pub use crate::rules::*;
pub use crate::tables::*;
