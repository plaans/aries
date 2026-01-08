//! This module contains extension traits to [Model](crate::model::Model) and [Domains] that
//! when imported provide convenience methods.
//!
//! - [DisjunctionExt] allows querying the value of a disjunction,
//!   whether it is currently unit, ...
//! - [AssignmentExt] provides methods to query the value of expressions.

mod assignments;
mod disjunction;
mod format;
pub mod partial_assignment;

pub use assignments::*;
pub use disjunction::*;
pub use format::*;
use state::Term;

use crate::core::state::{Domains, IntDomain};
use crate::core::*;
use crate::model::lang::IAtom;

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

pub type SavedAssignment = Domains;

impl AssignmentExt for SavedAssignment {
    fn entails(&self, literal: Lit) -> bool {
        self.entails(literal)
    }

    fn var_domain(&self, int: impl Into<IAtom>) -> IntDomain {
        let int = int.into();
        let (lb, ub) = self.bounds(int.var);
        IntDomain {
            lb: lb + int.shift,
            ub: ub + int.shift,
        }
    }

    fn presence_literal(&self, variable: impl Term) -> Lit {
        self.presence(variable)
    }

    fn to_owned_assignment(&self) -> SavedAssignment {
        todo!()
    }
}
