use crate::traits::interpreter::Interpreter;

use super::{env::Env, value::Value};

/// Represents a parameter of an action, condition, effect, ...
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    /// The name of the parameter.
    name: String,
    /// The type of the parameter.
    r#type: String,
    /// The value of the parameter when it is instantiated.
    value: Value,
}

impl Parameter {
    pub fn new(name: String, r#type: String, value: Value) -> Self {
        Parameter { name, r#type, value }
    }

    pub fn bound<E: Interpreter>(&self, env: &mut Env<E>) {
        env.bound(self.r#type.clone(), self.name.clone(), self.value.clone());
    }
}
