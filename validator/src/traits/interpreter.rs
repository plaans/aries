use anyhow::Result;

use crate::models::{env::Env, value::Value};

/// Represents an expression which can be interpreted.
pub trait Interpreter: Sized {
    /// Evaluates the expression with the environment.
    fn eval(&self, env: &Env<Self>) -> Result<Value>;
}
