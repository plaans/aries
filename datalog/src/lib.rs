#![warn(missing_docs)]
//#![warn(rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../README.md")]
mod logic;
mod program;
mod rules;

pub use crate::logic::*;
pub use crate::program::*;
pub use crate::rules::*;
