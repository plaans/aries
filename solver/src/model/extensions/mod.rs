//! This module contains extension traits to [Model](crate::model::Model) and [Domains] that
//! when imported provide convenience methods.
//!
//! - [DisjunctionExt] allows querying the value of a disjunction,
//!   whether it is currently unit, ...
//! - [AssignmentExt] provides methods to query the value of expressions.

mod disjunction;
mod domains_ext;
mod format;
pub mod partial_assignment;

pub use disjunction::*;
pub use domains_ext::*;
pub use format::*;

use crate::core::state::Domains;
use crate::core::*;

pub trait PartialBoolAssignment {
    fn entails(&self, literal: Lit) -> bool;
    fn value(&self, literal: Lit) -> Option<bool> {
        if self.entails(literal) {
            Some(true)
        } else if self.entails(!literal) {
            Some(false)
        } else {
            None
        }
    }

    fn presence_literal(&self, variable: VarRef) -> Lit;
}

impl PartialBoolAssignment for Domains {
    fn entails(&self, literal: Lit) -> bool {
        self.entails(literal)
    }

    fn presence_literal(&self, variable: VarRef) -> Lit {
        self.presence(variable)
    }
}

//TODO: remove
pub type SavedAssignment = Domains;
