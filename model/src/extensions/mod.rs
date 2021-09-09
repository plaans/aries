//! This module contains extension traits to [Model] and [OptDomains] that
//! when imported provide convenience methods.
//!
//! - [DisjunctionExt] allows querying the value of a disjunction,
//! whether it is currently unit, ...
//! - [ExpressionFactoryExt] provides method to create expressions in a given [Model]
//! - [AssignmentExt] provides methods to query the value of expressions.

mod assignments;
mod disjunction;
mod expression_factory;
mod format;

pub use assignments::*;
pub use disjunction::*;
pub use expression_factory::*;
pub use format::*;

use crate::bounds::Lit;
use crate::state::Domains;
use crate::Model;

pub trait PartialAssignment {
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
}

impl PartialAssignment for Domains {
    fn entails(&self, literal: Lit) -> bool {
        self.entails(literal)
    }
}

// TODO: this is correct but wasteful
//       also, it should be moved to state
pub type SavedAssignment = Model;

impl SavedAssignment {
    pub fn from_model(model: &Model) -> SavedAssignment {
        model.clone()
    }
}
