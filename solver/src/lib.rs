extern crate aries_model;

pub mod clauses;
pub(crate) mod cpu_time;
pub mod parallel_solver;
pub mod signals;
pub mod solver;
pub mod theories;

use crate::solver::BindingResult;
use aries_backtrack::{Backtrack, DecLvl};
use aries_model::bounds::Lit;
use aries_model::lang::reification::Expr;
use aries_model::state::{Domains, Explanation, InvalidUpdate};
use aries_model::{Model, WriterId};

/// A trait that provides the ability to bind an arbitrary expression to a literal.
pub trait Bind {
    /// When invoke, the module should add constraints to enforce `lit <=> expr`.
    ///
    /// The return value should provide feedback on whether it succeeded or failed to do so.
    fn bind(&mut self, literal: Lit, expr: &Expr, i: &mut Model) -> BindingResult;
}

/// A convenience trait that when implemented  will allow deriving the [Bind] trait.
pub trait BindSplit {
    fn enforce_true(&mut self, expr: &Expr, model: &mut Model) -> BindingResult;
    fn enforce_false(&mut self, expr: &Expr, model: &mut Model) -> BindingResult;
    fn enforce_eq(&mut self, literal: Lit, expr: &Expr, model: &mut Model) -> BindingResult;
}

impl<T: BindSplit> Bind for T {
    fn bind(&mut self, literal: Lit, expr: &Expr, i: &mut Model) -> BindingResult {
        debug_assert_eq!(i.state.current_decision_level(), DecLvl::ROOT);
        match i.state.value(literal) {
            Some(true) => self.enforce_true(expr, i),
            Some(false) => self.enforce_false(expr, i),
            None => self.enforce_eq(literal, expr, i),
        }
    }
}

pub trait Theory: Backtrack + Bind + Send + 'static {
    fn identity(&self) -> WriterId;

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
