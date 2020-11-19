use crate::queues::{QReader, QWriter, Q};
use crate::{Theory, TheoryStatus};
use aries_sat::all::Lit;

pub struct ModularSMT {
    literal_bindings: Q<Lit>,
    sat: SatSolver,
    theories: Vec<TheoryModule>,
}

pub struct SatSolver {
    output: QWriter<Lit>,
    sat: aries_sat::solver::Solver,
}

pub struct TheoryModule {
    input: QReader<Lit>,
    mapping: crate::solver::Mapping,
    theory: Box<dyn Theory>,
}

pub enum TheoryResult {
    Consistent,
    Contradiction(Vec<Lit>),
}

impl TheoryModule {
    pub fn process(&mut self) -> TheoryResult {
        for lit in &mut self.input {
            for atom in self.mapping.atoms_of(lit) {
                self.theory.enable(*atom)
            }
        }
        match self.theory.deduce() {
            TheoryStatus::Consistent => TheoryResult::Consistent,
            TheoryStatus::Inconsistent(atoms) => {
                let clause = atoms.iter().filter_map(|atom| self.mapping.literal_of(*atom)).collect();
                TheoryResult::Contradiction(clause)
            }
        }
    }
}
