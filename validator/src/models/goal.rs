use anyhow::Result;

use crate::traits::interpreter::Interpreter;

use super::{env::Env, value::Value};

#[derive(Debug)]
/// Represents an objective of the problem.
pub struct Goal<E: Interpreter>(E);

impl<E: Interpreter> From<E> for Goal<E> {
    fn from(e: E) -> Self {
        Self(e)
    }
}

impl<E: Interpreter> Goal<E> {
    pub fn eval(&self, env: &Env<E>) -> Result<Value> {
        self.0.eval(env)
    }

    pub fn expr(&self) -> &E {
        &self.0
    }
}
