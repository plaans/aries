//! Flatzinc problem solving using aries.

mod post;
mod solver;

pub mod constraint;

use aries::solver::search::SearchControl;
pub use post::Post;
pub use solver::Solver;

pub type Brancher = Box<dyn SearchControl<usize> + Send>;
