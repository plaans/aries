//! Flatzinc problem solving using aries.

mod config;
mod post;
mod solver;

pub mod constraint;

pub use config::Config;
pub use post::Post;
pub use solver::Solver;
