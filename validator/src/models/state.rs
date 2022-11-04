use std::collections::HashMap;

use anyhow::{Context, Result};

use super::value::Value;

/// Represents the current state of the world during the validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    fluents: HashMap<Vec<Value>, Value>,
}

impl State {
    /// Bounds a fluent to its current value.
    pub fn bound(&mut self, f: Vec<Value>, v: Value) {
        self.fluents.insert(f, v);
    }

    /// Returns the fluent corresponding to the given signature.
    pub fn get_fluent(&self, f: &Vec<Value>) -> Result<Value> {
        self.fluents
            .get(f)
            .context(format!("Unbounded fluent {:?}", f))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default() -> Result<()> {
        let state = State::default();
        assert!(state.fluents.is_empty());
        Ok(())
    }

    #[test]
    fn bound() -> Result<()> {
        let mut state = State::default();
        state.bound(vec![Value::Symbol("s".into())], Value::Bool(true));
        assert_eq!(
            state.fluents,
            HashMap::<Vec<Value>, Value>::from([(vec![Value::Symbol("s".into())], Value::Bool(true))])
        );
        Ok(())
    }

    #[test]
    fn get_fluent() -> Result<()> {
        let mut state = State::default();
        state.bound(vec![Value::Symbol("s".into())], Value::Bool(true));
        assert_eq!(state.get_fluent(&vec![Value::Symbol("s".into())])?, Value::Bool(true));
        assert!(state.get_fluent(&vec![Value::Symbol("a".into())]).is_err());
        Ok(())
    }
}
