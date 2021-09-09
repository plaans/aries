pub mod clauses;
pub(crate) mod cpu_time;
pub mod parallel_solver;
pub mod signals;
pub mod solver;
pub mod theories;

use crate::solver::{Binding, BindingResult};
use aries_backtrack::Backtrack;
use aries_backtrack::ObsTrail;
use aries_model::{Model, WriterId};

use aries_model::bounds::Lit;
use aries_model::expressions::ExprHandle;
use aries_model::state::{Domains, Explanation, InvalidUpdate};

pub trait Theory: Backtrack + Send + 'static {
    fn identity(&self) -> WriterId;

    fn bind(&mut self, literal: Lit, expr: ExprHandle, i: &mut Model, queue: &mut ObsTrail<Binding>) -> BindingResult;
    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction>;

    fn explain(&mut self, literal: Lit, context: u32, model: &Domains, out_explanation: &mut Explanation);

    fn print_stats(&self);

    fn clone_box(&self) -> Box<dyn Theory>;
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
