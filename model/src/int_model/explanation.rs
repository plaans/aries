use crate::bounds::Bound;
use crate::int_model::{DiscreteModel, InferenceCause};

/// Builder for a conjunction of literals that make the explained literal true
#[derive(Clone, Debug)]
pub struct Explanation {
    pub(crate) lits: Vec<Bound>,
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
    pub fn push(&mut self, lit: Bound) {
        self.lits.push(lit)
    }

    pub fn clear(&mut self) {
        self.lits.clear();
    }

    pub fn literals(&self) -> &[Bound] {
        &self.lits
    }
}
impl Default for Explanation {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Explainer {
    fn explain(&mut self, cause: InferenceCause, literal: Bound, model: &DiscreteModel, explanation: &mut Explanation);
}
