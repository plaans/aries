use crate::int_model::{DiscreteModel, InferenceCause};
use crate::lang::Bound;

/// Builder for a conjunction of literals that make the explained literal true
#[derive(Debug)]
pub struct Explanation {
    pub(crate) lits: Vec<Bound>,
}
impl Explanation {
    pub fn new() -> Self {
        Explanation { lits: Vec::new() }
    }
    pub fn push(&mut self, lit: Bound) {
        self.lits.push(lit)
    }
}
impl Default for Explanation {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Explainer {
    fn explain(&self, cause: InferenceCause, literal: Bound, model: &DiscreteModel, explanation: &mut Explanation);
}
