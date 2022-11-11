use anyhow::Result;

use crate::traits::interpreter::Interpreter;

use super::env::Env;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Represents a condition of an Action.
pub struct Condition<E: Interpreter>(E);

impl<E: Interpreter> From<E> for Condition<E> {
    fn from(e: E) -> Self {
        Self(e)
    }
}

impl<E: Interpreter> Condition<E> {
    /// Whether or not the condition is valid in the current environment.
    pub fn is_valid(&self, env: &Env<E>) -> Result<bool> {
        Ok(self.0.eval(env)? == true.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::models::value::Value;

    use super::*;

    struct MockExpr(Value);
    impl Default for MockExpr {
        fn default() -> Self {
            Self(true.into())
        }
    }
    impl Interpreter for MockExpr {
        fn eval(&self, _: &Env<Self>) -> Result<Value> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn is_valid() -> Result<()> {
        let env = Env::default();
        let t = Condition(MockExpr(true.into()));
        let f = Condition(MockExpr(false.into()));

        assert!(t.is_valid(&env)?);
        assert!(!f.is_valid(&env)?);
        Ok(())
    }
}
