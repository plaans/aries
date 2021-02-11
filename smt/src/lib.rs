pub mod clauses;
pub mod solver;
pub mod theories;

use crate::solver::{Binding, BindingResult};
use aries_backtrack::Backtrack;
use aries_backtrack::ObsTrail;
use aries_model::{Model, WModel};

use aries_model::expressions::ExprHandle;
use aries_model::int_model::Explanation;
use aries_model::lang::{Bound, VarRef};

#[derive(Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct AtomID {
    base_id: u32,
    negated: bool,
}
impl AtomID {
    pub fn new(base_id: u32, negated: bool) -> AtomID {
        AtomID { base_id, negated }
    }
    pub fn base_id(self) -> u32 {
        self.base_id
    }
    pub fn is_negated(self) -> bool {
        self.negated
    }
}
impl std::ops::Not for AtomID {
    type Output = Self;

    fn not(self) -> Self::Output {
        AtomID::new(self.base_id(), !self.is_negated())
    }
}

// pub trait SMTProblem<Literal: SatLiteral, Atom>: SatProblem<Literal> {
//     fn literal_of(&mut self, atom: Atom) -> Literal;
// }

pub trait Theory: Backtrack {
    fn bind(&mut self, literal: Bound, expr: ExprHandle, i: &mut Model, queue: &mut ObsTrail<Binding>)
        -> BindingResult;
    fn propagate(&mut self, model: &mut WModel) -> Result<(), Contradiction>;

    fn print_stats(&self);
}

pub enum Contradiction {
    EmptyDomain(VarRef),
    Explanation(Explanation),
}

// /// Represents the possibility of transforming an atom (Self) as Literal in T
// /// This trait derived for any Atom such that T = SMTProblem<Literal, Atom>
// /// Its purpose is to provide syntactic sugar to transform atoms into literals:
// /// `(atom: Atom).embed(solver): Literal
// pub trait Embeddable<T, Literal> {
//     /// Member method to embed an atom `self` into an SMTProblem.
//     fn embed(self, context: &mut T) -> Literal;
// }
//
// impl<Atom, Literal: SatLiteral, T: SMTProblem<Literal, Atom>> Embeddable<T, Literal> for Atom {
//     fn embed(self, context: &mut T) -> Literal {
//         context.literal_of(self)
//     }
// }

/// Result of recording an Atom.
/// Contains the atom's id and a boolean flag indicating whether the recording
/// resulted in a new id.
pub enum AtomRecording {
    Created(AtomID),
    Unified(AtomID),
    Tautology,
    Contradiction,
}

pub enum TheoryStatus {
    Consistent,                // TODO: theory implications
    Inconsistent(Vec<AtomID>), // TODO: reference to avoid allocation
}
