//! This module contains extension traits to [Model] and [Domains] that
//! when imported provide convenience methods.
//!
//! - [DisjunctionExt] allows querying the value of a disjunction,
//! whether it is currently unit, ...
//! - [AssignmentExt] provides methods to query the value of expressions.

mod assignments;
mod disjunction;
mod format;

pub use assignments::*;
pub use disjunction::*;
pub use format::*;

use crate::bounds::Lit;
use crate::lang::{IAtom, VarRef};
use crate::state::{Domains, IntDomain};
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

pub type SavedAssignment = Domains;

impl SavedAssignment {
    pub fn from_model<Lbl>(model: &Model<Lbl>) -> SavedAssignment {
        model.state.clone()
    }
}

impl AssignmentExt for SavedAssignment {
    fn entails(&self, literal: Lit) -> bool {
        self.entails(literal)
    }

    fn var_domain(&self, int: impl Into<IAtom>) -> IntDomain {
        let int = int.into();
        let (lb, ub) = self.bounds(int.var.into());
        IntDomain {
            lb: lb + int.shift,
            ub: ub + int.shift,
        }
    }

    fn presence_literal(&self, variable: VarRef) -> Lit {
        self.presence(variable)
    }

    fn to_owned_assignment(&self) -> SavedAssignment {
        todo!()
    }
}
