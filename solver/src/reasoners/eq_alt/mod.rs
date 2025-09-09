/// This module exports an alternate propagator for equality logic.
///
/// Since DenseEqTheory has O(n^2) space complexity it tends to have performance issues on larger problems.
/// This alternative has much lower memory use on sparse problems, and can make stronger inferences than the STN
mod graph;
mod node;
mod propagators;
mod relation;
mod theory;

pub use theory::AltEqTheory;
