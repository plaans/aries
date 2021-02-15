pub mod clauses;
pub(crate) mod cpu_time;
pub mod solver;
pub mod theories;

use crate::solver::{Binding, BindingResult};
use aries_backtrack::Backtrack;
use aries_backtrack::ObsTrail;
use aries_model::{Model, WriterId};

use aries_model::bounds::Bound;
use aries_model::expressions::ExprHandle;
use aries_model::int_model::{DiscreteModel, EmptyDomain, Explanation};
use aries_model::lang::VarRef;

pub trait Theory: Backtrack {
    fn identity(&self) -> WriterId;

    fn bind(&mut self, literal: Bound, expr: ExprHandle, i: &mut Model, queue: &mut ObsTrail<Binding>)
        -> BindingResult;
    fn propagate(&mut self, model: &mut DiscreteModel) -> Result<(), Contradiction>;

    // TODO: use inner type instead of u64
    fn explain(&mut self, literal: Bound, context: u64, model: &DiscreteModel, out_explanation: &mut Explanation);

    fn print_stats(&self);
}

#[derive(Debug)]
pub enum Contradiction {
    EmptyDomain(VarRef),
    Explanation(Explanation),
}
impl From<EmptyDomain> for Contradiction {
    fn from(empty: EmptyDomain) -> Self {
        Contradiction::EmptyDomain(empty.0)
    }
}
impl From<Explanation> for Contradiction {
    fn from(e: Explanation) -> Self {
        Contradiction::Explanation(e)
    }
}
