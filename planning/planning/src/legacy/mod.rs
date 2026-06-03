//! This module contains API that were previously in aries_solver but where moved out.

mod atom;
mod cst;
mod expr;
mod fixed;
mod format;
mod linear_rational;
mod partial_assignment;
mod sym;
mod symbols;
mod types;
mod variables;
pub use atom::*;
pub use cst::*;
pub use expr::*;
pub use fixed::*;
pub use format::*;
pub use linear_rational::*;
pub use partial_assignment::*;
pub use sym::*;
pub use symbols::*;
pub use types::*;
pub use variables::*;

pub mod input;
pub mod utils;

pub use aries::lang::ConversionError;
use aries::model::Model;

use aries::prelude::DomainsExt;

pub fn unifiable<L>(model: &Model<L>, a: impl Into<Atom>, b: impl Into<Atom>) -> bool {
    let a = a.into();
    let b = b.into();
    if a.kind() != b.kind() {
        false
    } else {
        let (l1, u1) = model.bounds(a);
        let (l2, u2) = model.bounds(b);
        let disjoint = u1 < l2 || u2 < l1;
        !disjoint
    }
}

pub fn unifiable_seq<L, A: Into<Atom> + Copy, B: Into<Atom> + Copy>(model: &Model<L>, a: &[A], b: &[B]) -> bool {
    if a.len() != b.len() {
        false
    } else {
        for (a, b) in a.iter().zip(b.iter()) {
            let a = (*a).into();
            let b = (*b).into();
            if !unifiable(model, a, b) {
                return false;
            }
        }
        true
    }
}
