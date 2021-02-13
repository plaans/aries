pub mod clauses;
pub mod solver;
pub mod theories;

use crate::solver::{Binding, BindingResult};
use aries_backtrack::Backtrack;
use aries_backtrack::ObsTrail;
use aries_model::{Model, WModel};

use aries_model::expressions::ExprHandle;
use aries_model::int_model::{DiscreteModel, EmptyDomain, Explanation};
use aries_model::lang::{Bound, VarRef};

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

pub trait Theory: Backtrack {
    fn bind(&mut self, literal: Bound, expr: ExprHandle, i: &mut Model, queue: &mut ObsTrail<Binding>)
        -> BindingResult;
    fn propagate(&mut self, model: &mut WModel) -> Result<(), Contradiction>;

    // TODO: use inner type instead of u64
    fn explain(&mut self, literal: Bound, context: u64, model: &DiscreteModel, out_explanation: &mut Explanation);

    fn print_stats(&self);
}

pub enum Contradiction {
    EmptyDomain(VarRef),
    Explanation(Explanation),
}
impl From<EmptyDomain> for Contradiction {
    fn from(empty: EmptyDomain) -> Self {
        Contradiction::EmptyDomain(empty.0)
    }
}
