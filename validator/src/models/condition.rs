use anyhow::Result;

use super::{env::Env, expression::ValExpression, value::Value};

/// The minimal behaviour of a condition.
pub trait ValCondition {
    /// Returns the expression of the condition.
    fn expr(&self) -> Result<Box<dyn ValExpression>>;
    /// Checks if the condition is valid.
    fn is_valid(&self, env: &Env) -> Result<bool> {
        Ok(self.expr()?.eval(env)? == Value::Bool(true))
    }
}
