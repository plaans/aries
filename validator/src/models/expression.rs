use anyhow::Result;

use super::{env::Env, value::Value};

/// The minimal behaviour of an expression to validate a plan.
pub trait ValExpression {
    /// Returns the value corresponding to the expression after evaluation in the current environment.
    fn eval(&self, env: &Env) -> Result<Value>;
    /// Returns the symbol associated to this expression.
    fn symbol(&self) -> Result<String>;
    /// Returns the type associated to this expression.
    fn tpe(&self) -> Result<String>;
}
