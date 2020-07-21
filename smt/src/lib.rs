use aries_collections::id_map::IdMap;
use aries_sat::all::{BVal, BVar, Lit};
use aries_sat::SearchResult;
use std::collections::{HashMap, HashSet};

type AtomID = u32;

#[derive(Default)]
pub struct Mapping {
    atoms: HashMap<Lit, Vec<AtomID>>,
    literal: HashMap<AtomID, Lit>,
    empty_vec: Vec<AtomID>,
}
impl Mapping {
    pub fn bind(&mut self, lit: Lit, atom: AtomID) {
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
    Consistent, // todo: theory implications
    Inconsistent(Vec<AtomID>),
}

pub trait Theory<Atom> {
    fn record_atom(&mut self, atom: Atom) -> AtomID;
    fn enable(&mut self, atom_id: AtomID);
    fn deduce(&mut self) -> TheoryStatus;
    fn set_backtrack_point(&mut self);
    fn backtrack(&mut self);
}

pub struct SMTSolver<Atom, T: Theory<Atom>> {
    pub sat: aries_sat::Solver,
    pub theory: T,
    mapping: Mapping,
    atom: std::marker::PhantomData<Atom>,
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

    pub fn solve(&mut self) -> Option<Model> {
        lazy_dpll_t(&mut self.sat, &mut self.theory, &self.mapping)
    }
}

// TODO: remove of make more generic
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
