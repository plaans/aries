mod assignments;
mod disjunction;

use crate::bounds::Lit;
use crate::state::OptDomains;
use crate::Model;
pub use assignments::*;
pub use disjunction::*;

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

impl PartialAssignment for OptDomains {
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
