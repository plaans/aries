//! This module contains extension traits to [Model] and [Domains] that
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
use crate::lang::Expr;
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

pub trait Constraint {
    fn enforce(&self, model: &mut Model);
    fn reify(self, model: &mut Model) -> Lit;
}

impl Constraint for Lit {
    fn enforce(&self, model: &mut Model) {
        model.enforce(*self);
    }

    fn reify(self, _: &mut Model) -> Lit {
        self
    }
}
impl Constraint for Expr {
    fn enforce(&self, model: &mut Model) {
        model.enforce(self);
    }

    fn reify(self, model: &mut Model) -> Lit {
        model.reify(self)
    }
}
