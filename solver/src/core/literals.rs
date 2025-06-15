//! Various datastructures specialized for the handling of literals (watchlists, sets, clauses, implication graph, ...)

pub use disjunction::*;
pub use implication_graph::*;
pub use lit_set::*;
pub use watches::*;

mod disjunction;
mod implication_graph;
mod lit_set;
mod watches;
