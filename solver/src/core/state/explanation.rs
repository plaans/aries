use crate::core::state::{Domains, InferenceCause};
use crate::core::Lit;

/// Builder for a conjunction of literals that make the explained literal true
#[derive(Clone, Debug)]
pub struct Explanation {
    pub lits: Vec<Lit>,
}
impl Explanation {
    pub fn new() -> Self {
        Explanation { lits: Vec::new() }
    }
    pub fn with_capacity(n: usize) -> Self {
        Explanation {
            lits: Vec::with_capacity(n),
        }
    }
    pub fn reserve(&mut self, additional: usize) {
        self.lits.reserve(additional)
    }
    pub fn push(&mut self, lit: Lit) {
        self.lits.push(lit)
    }
    pub fn pop(&mut self) -> Option<Lit> {
        self.lits.pop()
    }

    pub fn clear(&mut self) {
        self.lits.clear();
    }

    pub fn literals(&self) -> &[Lit] {
        &self.lits
    }
}
impl Default for Explanation {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Explainer {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &Domains, explanation: &mut Explanation);
}

/// A provides an explainer for a standalone theory. useful for testing purposes.
#[cfg(test)]
pub struct SingleTheoryExplainer<'a, T: crate::reasoners::Theory>(pub &'a mut T);

#[cfg(test)]
impl<'a, T: crate::reasoners::Theory> Explainer for SingleTheoryExplainer<'a, T> {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &Domains, explanation: &mut Explanation) {
        assert_eq!(cause.writer, self.0.identity());
        self.0.explain(literal, cause.payload, model, explanation)
    }
}
