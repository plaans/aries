pub mod diff_logic;
pub mod solver;

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

pub trait Theory<Atom> {
    fn record_atom(&mut self, atom: Atom) -> AtomRecording;
    fn enable(&mut self, atom_id: AtomID);
    fn deduce(&mut self) -> TheoryStatus;
    fn set_backtrack_point(&mut self) -> u32;
    fn get_last_backtrack_point(&mut self) -> u32;
    fn backtrack(&mut self);
    fn backtrack_to(&mut self, point: u32);
}

/// Result of recording an Atom.
/// Contains the atom's id and a boolean flag indicating whether the recording
/// resulted in a new id.
pub struct AtomRecording {
    created: bool,
    id: AtomID,
}
impl AtomRecording {
    pub fn newly_created(id: AtomID) -> AtomRecording {
        AtomRecording { created: true, id }
    }
    pub fn unified_with_existing(id: AtomID) -> AtomRecording {
        AtomRecording { created: false, id }
    }
}

pub enum TheoryStatus {
    Consistent,                // TODO: theory implications
    Inconsistent(Vec<AtomID>), // TODO: reference to avoid allocation
}
