extern crate aries_model;

use aries_backtrack::{Backtrack, DecLvl};
use aries_core::state::{Domains, Explanation, InvalidUpdate};
use aries_core::*;
use aries_model::lang::reification::Expr;

use crate::solver::BindingResult;

pub mod clauses;
pub(crate) mod cpu_time;
pub mod parallel_solver;
pub mod signals;
pub mod solver;
pub mod theories;

/// A trait that provides the ability to bind an arbitrary expression to a literal.
pub trait Bind {
    /// When invoke, the module should add constraints to enforce `lit <=> expr`.
    ///
    /// The return value should provide feedback on whether it succeeded or failed to do so.
    fn bind(&mut self, literal: Lit, expr: &Expr, doms: &mut Domains) -> BindingResult;
}

/// A convenience trait that when implemented  will allow deriving the [Bind] trait.
pub trait BindSplit {
    fn enforce_true(&mut self, expr: &Expr, doms: &mut Domains) -> BindingResult;
    fn enforce_false(&mut self, expr: &Expr, doms: &mut Domains) -> BindingResult;
    fn enforce_eq(&mut self, literal: Lit, expr: &Expr, doms: &mut Domains) -> BindingResult;
}

impl<T: BindSplit> Bind for T {
    fn bind(&mut self, literal: Lit, expr: &Expr, doms: &mut Domains) -> BindingResult {
        debug_assert_eq!(doms.current_decision_level(), DecLvl::ROOT);
        match doms.value(literal) {
            Some(true) => self.enforce_true(expr, doms),
            Some(false) => self.enforce_false(expr, doms),
            None => self.enforce_eq(literal, expr, doms),
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
