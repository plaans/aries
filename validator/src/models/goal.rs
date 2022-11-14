use std::slice::Iter;

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

/// Represents an iterator of goals.
pub struct GoalIter<E: Interpreter>(Vec<Goal<E>>);

impl<E: Interpreter> From<Vec<Goal<E>>> for GoalIter<E> {
    fn from(g: Vec<Goal<E>>) -> Self {
        Self(g)
    }
}

impl<E: Interpreter> GoalIter<E> {
    pub fn iter(&self) -> Iter<'_, Goal<E>> {
        self.0.iter()
    }
}
