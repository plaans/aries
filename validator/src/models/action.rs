use anyhow::Result;
use dyn_clone::DynClone;

use super::{condition::ValCondition, effect::ValEffect, expression::ValExpression};

/// The minimal behaviour of a plan action.
pub trait ValAction: DynClone {
    /// Returns the preconditions of the action.
    fn conditions(&self) -> Result<Vec<Box<dyn ValCondition>>>;
    /// Returns the effects of the action.
    fn effects(&self) -> Result<Vec<Box<dyn ValEffect>>>;
    /// Returns the name of the action.
    fn name(&self) -> Result<String>;
    /// Returns the parameters of the action.
    fn parameters(&self) -> Result<Vec<Box<dyn ValExpression>>>;
}
