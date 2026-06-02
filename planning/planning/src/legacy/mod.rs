//! This module contains API that were previously in aries_solver but where moved out.

mod atom;
mod cst;
mod expr;
mod format;
mod partial_assignment;

pub use aries::model::lang::{ConversionError, FAtom, FVar, Kind, Rational, SAtom, SVar, Type, Variable};
pub use aries::model::symbols::*;
pub use aries::model::types::*;
use aries::model::Model;

use aries::prelude::DomainsExt;
pub use atom::*;
pub use cst::*;
pub use expr::*;
pub use format::*;
pub use partial_assignment::*;

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
