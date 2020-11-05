pub mod all;
pub mod clause;
pub mod cnf;
pub mod heuristic;
pub mod solver;
pub mod stats;

/// Trait that enforces minimal capabilities for a sat literal.
/// The trait is automatically derived for any type that fulfills all requirements.
pub trait SatLiteral: Copy + std::ops::Not<Output = Self> {}
impl<X> SatLiteral for X where X: Copy + std::ops::Not<Output = Self> {}

/// Trait with minimal methods to build sat problem in a solver independent way.
pub trait SatProblem<Literal: SatLiteral> {
    fn new_variable(&mut self) -> Literal;
    fn add_clause(&mut self, disjuncts: &[Literal]);

    /// A literal that is always true in a valid model.
    fn tautology(&mut self) -> Literal;

    /// A literal that is always false in a valid model
    fn contradiction(&mut self) -> Literal {
        !self.tautology()
    }

    fn enforce(&mut self, literal: Literal) {
        self.add_clause(&[literal])
    }

    fn enforce_either(&mut self, option1: Literal, option2: Literal) {
        self.add_clause(&[option1, option2])
    }

    fn reified_or(&mut self, disjuncts: &[Literal]) -> Literal {
        match disjuncts.len() {
            0 => self.contradiction(),
            1 => disjuncts[0],
            _ => {
                let reif = self.new_variable();
                let mut clause = Vec::with_capacity(disjuncts.len() + 1);
                // make reif => disjuncts
                clause.push(!reif);
                disjuncts.iter().for_each(|l| clause.push(*l));
                self.add_clause(&clause);
                for &disjunct in disjuncts {
                    // enforce disjunct => reif
                    clause.clear();
                    clause.push(!disjunct);
                    clause.push(reif);
                    self.add_clause(&clause);
                }
                reif
            }
        }
    }
}
