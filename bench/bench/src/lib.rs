pub use aries_bench_data::*;

pub mod aggregator;
pub mod comp;
pub mod metric;
pub mod plot;
pub mod results;
pub mod table;
pub mod time_series;

/// Identifier of solver (typically a string derive from the location of its results)
pub type SolverID = String;
