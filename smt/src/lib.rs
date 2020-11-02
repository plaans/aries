pub mod model;

use crate::model::Literal;
use aries_collections::id_map::IdMap;
use aries_sat::all::{BVal, BVar, Lit};
use aries_sat::{ConflictHandlingResult, PropagationResult, SearchResult};
use std::collections::{HashMap, HashSet};
use std::ops::Not;

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

#[derive(Default)]
pub struct Mapping {
    atoms: HashMap<Lit, Vec<AtomID>>,
    literal: HashMap<AtomID, Lit>,
    empty_vec: Vec<AtomID>,
}
impl Mapping {
    pub fn bind(&mut self, lit: Lit, atom: impl Into<AtomID>) {
        let atom: AtomID = atom.into();
        assert!(!self.literal.contains_key(&atom));
        self.literal.insert(atom, lit);
        self.atoms
            .entry(lit)
            .or_insert_with(|| Vec::with_capacity(1))
            .push(atom);
    }
}
impl LiteralAtomMapping for Mapping {
    fn atoms_of(&self, lit: Lit) -> &[AtomID] {
        self.atoms.get(&lit).unwrap_or(&self.empty_vec)
    }

    fn literal_of(&self, atom: AtomID) -> Option<Lit> {
        self.literal.get(&atom).copied()
    }
}

trait LiteralAtomMapping {
    fn atoms_of(&self, lit: aries_sat::all::Lit) -> &[AtomID];
    fn literal_of(&self, atom: AtomID) -> Option<Lit>;
}

pub enum TheoryStatus {
    Consistent,                // todo: theory implications
    Inconsistent(Vec<AtomID>), //TODO: reference to avoid allocation
}

pub trait Theory<Atom> {
    fn record_atom(&mut self, atom: Atom) -> AtomID;
    fn enable(&mut self, atom_id: AtomID);
    fn deduce(&mut self) -> TheoryStatus;
    fn set_backtrack_point(&mut self) -> u32;
    fn get_last_backtrack_point(&mut self) -> u32;
    fn backtrack(&mut self);
    fn backtrack_to(&mut self, point: u32);
}

pub struct SMTSolver<Atom, T: Theory<Atom>> {
    pub sat: aries_sat::Solver,
    pub theory: T,
    mapping: Mapping,
    atom: std::marker::PhantomData<Atom>,
}

impl<Atom, T: Theory<Atom> + Default> Default for SMTSolver<Atom, T> {
    fn default() -> Self {
        SMTSolver {
            sat: aries_sat::Solver::default(),
            theory: T::default(),
            mapping: Default::default(),
            atom: Default::default(),
        }
    }
}

enum SmtLit<TheoryAtom> {
    Sat(aries_sat::all::Lit),
    AtomID(AtomID),
    RawAtom(TheoryAtom),
}
impl<TheoryAtom> From<TheoryAtom> for SmtLit<TheoryAtom> {
    fn from(ta: TheoryAtom) -> Self {
        SmtLit::RawAtom(ta)
    }
}
impl<X> From<Lit> for SmtLit<X> {
    fn from(lit: Lit) -> Self {
        SmtLit::Sat(lit)
    }
}
impl<X> From<AtomID> for SmtLit<X> {
    fn from(atom: AtomID) -> Self {
        SmtLit::AtomID(atom)
    }
}

impl<Atom, T: Theory<Atom>> SMTSolver<Atom, T> {
    pub fn new(sat: aries_sat::Solver, theory: T, mapping: Mapping) -> Self {
        SMTSolver {
            sat,
            theory,
            mapping,
            atom: Default::default(),
        }
    }

    pub fn add_clause(&mut self, clause: &[SmtLit<Atom>]) {
        let mut sat_clause = clause
            .iter()
            .map(|sl| match sl {
                SmtLit::Sat(l) => l,
                SmtLit::AtomID(id) => smt.bindings.lit_of(*id),
                SmtLit::RawAtom(atom) => smt.get_lit(atom),
            })
            .collect();
        self.sat.add_clause(sat_clause);
    }

    fn get_var<A: Into<Atom>>(&mut self, atom: A) -> BVar {
        unimplemented!() // TODO: remove, we should directly map to a literal
    }
    fn get_lit<A: Into<Literal<Atom>>>(&mut self, lit: A) -> Lit {
        match lit.into() {
            Literal::Pos(v) => self.get_var(v).true_lit(),
            Literal::Neg(v) => self.get_var(v).false_lit(),
        }
    }

    pub fn solve(&mut self, lazy: bool) -> Option<Model> {
        if lazy {
            lazy_dpll_t(&mut self.sat, &mut self.theory, &self.mapping)
        } else {
            self.solve_eager()
        }
    }

    pub fn solve_eager(&mut self) -> Option<Model> {
        loop {
            match self.sat.propagate() {
                PropagationResult::Conflict(clause) => {
                    // we must handle conflict and backtrack in theory
                    match self.sat.handle_conflict(clause) {
                        ConflictHandlingResult::Backtracked {
                            num_backtracks,
                            inferred,
                        } => {
                            for _ in 0..num_backtracks.get() {
                                self.theory.backtrack();
                            }
                            for x in self.mapping.atoms_of(inferred) {
                                self.theory.enable(*x);
                            }
                        }
                        ConflictHandlingResult::Unsat => {
                            // UNSAT: nothing was left to undo
                            return None;
                        }
                    }
                }
                PropagationResult::Inferred(inferred_literals) => {
                    for &l in inferred_literals {
                        for &atom in self.mapping.atoms_of(l) {
                            self.theory.enable(atom);
                        }
                    }

                    match self.theory.deduce() {
                        TheoryStatus::Consistent => {
                            if let Some(decision) = self.sat.next_decision() {
                                // force decision
                                self.sat.decide(decision);
                                self.theory.set_backtrack_point();
                                for &atom in self.mapping.atoms_of(decision) {
                                    self.theory.enable(atom);
                                }
                            } else {
                                // Solution found
                                return Some(self.sat.model());
                            }
                        }
                        TheoryStatus::Inconsistent(culprits) => {
                            // create clause
                            debug_assert_eq!(
                                culprits.len(),
                                culprits.iter().collect::<HashSet<_>>().len(),
                                "Duplicated elements in the culprit set: {:?}",
                                culprits
                            );
                            let clause: Vec<Lit> = culprits
                                .iter()
                                .filter_map(|culprit| self.mapping.literal_of(*culprit).map(Lit::negate))
                                .collect();

                            // add clause excluding the current assignment to the solver
                            self.sat.add_forgettable_clause(&clause);
                        }
                    }
                }
            }
        }
    }
}

// TODO: remove or make more generic
type Model = IdMap<BVar, BVal>;

fn lazy_dpll_t<Atom, T: Theory<Atom>>(
    sat: &mut aries_sat::Solver,
    theory: &mut T,
    mapping: &impl LiteralAtomMapping,
) -> Option<Model> {
    theory.set_backtrack_point();
    loop {
        match sat.solve() {
            SearchResult::Unsolvable => return None,
            SearchResult::Abandoned(_) => unreachable!(),
            SearchResult::Solved(m) => {
                theory.backtrack();
                theory.set_backtrack_point();

                // activate theory constraints based on model
                // literals are processed in the order they were set in the SAT solver to ensure
                // that an incremental handling in the theory will return a conflict based on the
                // smallest decision level possible
                for literal in m.set_literals() {
                    for atom in mapping.atoms_of(literal) {
                        theory.enable(*atom);
                    }
                }
                match theory.deduce() {
                    TheoryStatus::Consistent => {
                        // we have a new solution
                        return Some(sat.model());
                    }
                    TheoryStatus::Inconsistent(culprits) => {
                        debug_assert_eq!(
                            culprits.len(),
                            culprits.iter().collect::<HashSet<_>>().len(),
                            "Duplicated elements in the culprit set: {:?}",
                            culprits
                        );
                        let clause: Vec<Lit> = culprits
                            .iter()
                            .filter_map(|culprit| mapping.literal_of(*culprit).map(Lit::negate))
                            .collect();

                        // add clause excluding the current assignment to the solver
                        sat.add_forgettable_clause(&clause);
                    }
                }
            }
        }
    }
}
