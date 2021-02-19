pub mod clauses;
pub(crate) mod cpu_time;
pub mod signals;
pub mod solver;
pub mod theories;

use crate::solver::{Binding, BindingResult};
use aries_backtrack::Backtrack;
use aries_backtrack::ObsTrail;
use aries_model::{Model, WriterId};

use aries_model::bounds::Bound;
use aries_model::expressions::ExprHandle;
use aries_model::int_model::{DiscreteModel, Explanation, InvalidUpdate};

pub trait Theory: Backtrack {
    fn identity(&self) -> WriterId;

    fn bind(&mut self, literal: Bound, expr: ExprHandle, i: &mut Model, queue: &mut ObsTrail<Binding>)
        -> BindingResult;
    fn propagate(&mut self, model: &mut DiscreteModel) -> Result<(), Contradiction>;

    fn explain(&mut self, literal: Bound, context: u32, model: &DiscreteModel, out_explanation: &mut Explanation);

    fn print_stats(&self);
}

#[derive(Debug)]
pub enum Contradiction {
    InvalidUpdate(InvalidUpdate),
    Explanation(Explanation),
}
impl From<InvalidUpdate> for Contradiction {
    fn from(empty: InvalidUpdate) -> Self {
        Contradiction::InvalidUpdate(empty)
    }
}
impl From<Explanation> for Contradiction {
    fn from(e: Explanation) -> Self {
        Contradiction::Explanation(e)
    }
}
