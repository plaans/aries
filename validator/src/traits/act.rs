use anyhow::Result;

use crate::models::{condition::SpanCondition, env::Env, state::State};

use super::interpreter::Interpreter;

/// Represents a structure which can affect the current State.
pub trait Act<E: Interpreter> {
    /// Returns the list of condition to affect the State.
    fn conditions(&self) -> &Vec<SpanCondition<E>>;
    /// Affects the state only if the application is possible.
    fn apply(&self, env: &Env<E>, s: &State) -> Result<Option<State>>;

    /// Returns whether or not the application is possible.
    fn applicable(&self, env: &Env<E>) -> Result<bool> {
        for c in self.conditions() {
            if !c.is_valid(env)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::models::value::Value;

    use super::*;

    #[derive(Clone)]
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

    struct MockAct(Vec<SpanCondition<MockExpr>>, Vec<Value>, Value);
    impl Act<MockExpr> for MockAct {
        fn conditions(&self) -> &Vec<SpanCondition<MockExpr>> {
            &self.0
        }

        fn apply(&self, _env: &Env<MockExpr>, _s: &State) -> Result<Option<State>> {
            todo!()
        }
    }

    fn can_apply() -> MockAct {
        MockAct(
            vec![
                SpanCondition::new(MockExpr(true.into()), vec![]),
                SpanCondition::new(MockExpr(true.into()), vec![]),
            ],
            vec!["s".into()],
            true.into(),
        )
    }
    fn cannot_apply() -> MockAct {
        MockAct(
            vec![
                SpanCondition::new(MockExpr(false.into()), vec![]),
                SpanCondition::new(MockExpr(true.into()), vec![]),
            ],
            vec!["s".into()],
            true.into(),
        )
    }

    #[test]
    fn applicable() -> Result<()> {
        let env = Env::default();
        let t = can_apply();
        let f = cannot_apply();

        assert!(t.applicable(&env)?);
        assert!(!f.applicable(&env)?);
        Ok(())
    }
}
