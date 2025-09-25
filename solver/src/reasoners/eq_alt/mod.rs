//! This module exports an alternate propagator for equality logic.
//!
//! Since DenseEqTheory has O(n^2) space complexity it tends to have performance issues on larger problems.
//! This alternative has much lower memory use on sparse problems, and can make stronger inferences than just the STN
//!
//! Currently, this propagator is intended to be used in conjunction with the StnTheory.
//! Each l => x = y constraint should be posted as l => x >= y and l => x <= y,
//! and each l => x != y constraint should be posted as l => x > y or l => x < y in the STN.
//! This is because AltEqTheory does not do bound propagation yet
//! (When a integer variable's bounds are updated, no propagation occurs).
//! Stn is therefore ideally used in "bounds" propagation mode ("edges" is redundant) with this propagator.

// TODO: Implement bound propagation for this theory.

mod constraints;
mod graph;
mod node;
mod relation;
mod theory;

pub use theory::AltEqTheory;
