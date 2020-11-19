use crate::*;

#[derive(Default)]
pub(crate) struct Mapping {
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

    pub fn atoms_of(&self, lit: Lit) -> &[AtomID] {
        self.atoms.get(&lit).unwrap_or(&self.empty_vec)
    }

    pub fn literal_of(&self, atom: AtomID) -> Option<Lit> {
        self.literal.get(&atom).copied()
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

// TODO: is this really useful
trait LiteralAtomMapping {
    fn atoms_of(&self, lit: aries_sat::all::Lit) -> &[AtomID];
    fn literal_of(&self, atom: AtomID) -> Option<Lit>;
}

pub struct SMTSolver<Atom, T: DynamicTheory<Atom>> {
    pub sat: aries_sat::solver::Solver,
    pub theory: T,
    mapping: Mapping,
    tautology: Lit,
    atom: std::marker::PhantomData<Atom>,
}

impl<Atom, T: DynamicTheory<Atom>> SMTProblem<Lit, Atom> for SMTSolver<Atom, T> {
    fn literal_of(&mut self, atom: Atom) -> Lit {
        match self.theory.record_atom(atom) {
            AtomRecording::Created(id) => {
                let lit = self.sat.add_var().true_lit();
                self.mapping.bind(lit, id);
                self.mapping.bind(!lit, !id);
                lit
            }
            AtomRecording::Unified(id) => self.literal_of_id(id).unwrap(),
            AtomRecording::Tautology => self.tautology(),
            AtomRecording::Contradiction => self.contradiction(),
        }
    }
}

impl<Atom, T: DynamicTheory<Atom>> SatProblem<Lit> for SMTSolver<Atom, T> {
    fn new_variable(&mut self) -> Lit {
        self.sat.add_var().true_lit()
    }

    fn add_clause(&mut self, disjuncts: &[Lit]) {
        self.sat.add_clause(disjuncts);
    }

    fn tautology(&mut self) -> Lit {
        self.tautology
    }
}

impl<Atom, T: DynamicTheory<Atom> + Default> Default for SMTSolver<Atom, T> {
    fn default() -> Self {
        let mut sat = aries_sat::solver::Solver::default();
        let tautology = sat.add_var().true_lit();
        sat.add_clause(&[tautology]);
        SMTSolver {
            sat,
            theory: T::default(),
            mapping: Default::default(),
            tautology,
            atom: Default::default(),
        }
    }
}

// TODO: remove ?
pub enum SmtLit<TheoryAtom> {
    Sat(aries_sat::all::Lit),
    AtomID(AtomID),
    RawAtom(TheoryAtom),
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

// TODO: remove or make more generic
type Model = IdMap<BVar, BVal>;

impl<Atom, T: DynamicTheory<Atom>> SMTSolver<Atom, T> {
    pub fn literal_of_id(&mut self, atom: AtomID) -> Option<Lit> {
        self.mapping.literal_of(atom)
    }

    pub fn solve(&mut self, lazy: bool) -> Option<Model> {
        if lazy {
            self.solve_lazy()
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

    fn solve_lazy(&mut self) -> Option<Model> {
        self.theory.set_backtrack_point();
        loop {
            match self.sat.solve() {
                SearchResult::Unsolvable => return None,
                SearchResult::Abandoned(_) => unreachable!(),
                SearchResult::Solved(m) => {
                    self.theory.backtrack();
                    self.theory.set_backtrack_point();

                    // activate theory constraints based on model
                    // literals are processed in the order they were set in the SAT solver to ensure
                    // that an incremental handling in the theory will return a conflict based on the
                    // smallest decision level possible
                    for literal in m.set_literals() {
                        for atom in self.mapping.atoms_of(literal) {
                            self.theory.enable(*atom);
                        }
                    }
                    match self.theory.deduce() {
                        TheoryStatus::Consistent => {
                            // we have a new solution
                            return Some(self.sat.model());
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
