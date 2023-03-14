use anyhow::Result;

use crate::models::{csp::CspConstraint, env::Env, value::Value};

/// Represents an expression which can be interpreted.
pub trait Interpreter: Sized {
    /// Evaluates the expression with the environment as a Value.
    fn eval(&self, env: &Env<Self>) -> Result<Value>;

    /// Evaluates the expression with the environment as a CSP constraint.
    fn convert_to_csp_constraint(&self, env: &Env<Self>) -> Result<CspConstraint>;
}
