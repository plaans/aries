pub mod backtrack;
pub mod lang;
pub mod model;
pub mod modules;
pub mod queues;
pub mod solver;

use crate::backtrack::Backtrack;
use crate::lang::{BAtom, IVar, IntCst, Interner};
use crate::model::{ModelEvents, WModel, WriterId};
use crate::modules::{Binding, BindingResult, TheoryResult};
use crate::queues::{QReader, Q};
use aries_collections::id_map::IdMap;
use aries_sat::all::{BVal, BVar, Lit};
use aries_sat::solver::{ConflictHandlingResult, PropagationResult, SearchResult};
use aries_sat::{SatLiteral, SatProblem};
use std::collections::{HashMap, HashSet};

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

pub trait SMTProblem<Literal: SatLiteral, Atom>: SatProblem<Literal> {
    fn literal_of(&mut self, atom: Atom) -> Literal;
}

pub trait Theory: Backtrack {
    fn bind(&mut self, literal: Lit, atom: BAtom, i: &mut Interner, queue: &mut Q<Binding>) -> BindingResult;
    fn propagate(&mut self, events: &mut ModelEvents, model: &mut WModel) -> TheoryResult;
    // TODO: remove
    fn domain_of(&self, ivar: IVar) -> Option<(IntCst, IntCst)> {
        None
    }
    // TODO: can we remove this (and AtomID)
    fn enable(&mut self, atom_id: AtomID);
    fn deduce(&mut self) -> TheoryStatus;
}

// TODO: remove
pub trait DynamicTheory<Atom>: Theory {
    fn record_atom(&mut self, atom: Atom) -> AtomRecording;
}

/// Represents the possibility of transforming an atom (Self) as Literal in T
/// This trait derived for any Atom such that T = SMTProblem<Literal, Atom>
/// Its purpose is to provide syntactic sugar to transform atoms into literals:
/// `(atom: Atom).embed(solver): Literal
pub trait Embeddable<T, Literal> {
    /// Member method to embed an atom `self` into an SMTProblem.
    fn embed(self, context: &mut T) -> Literal;
}

impl<Atom, Literal: SatLiteral, T: SMTProblem<Literal, Atom>> Embeddable<T, Literal> for Atom {
    fn embed(self, context: &mut T) -> Literal {
        context.literal_of(self)
    }
}

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
