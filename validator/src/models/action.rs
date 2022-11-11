use std::slice::Iter;

use anyhow::Result;

use crate::traits::{act::Act, interpreter::Interpreter};

use super::{condition::Condition, effects::Effect, env::Env, state::State, value::Value};

#[derive(Debug, PartialEq, Eq)]
/// Representation of an action for the validation.
pub struct Action<E: Interpreter> {
    /// The name of the action.
    name: String,
    /// The list of conditions for the application of the action.
    conditions: Vec<Condition<E>>,
    /// The list of effects
    effects: Vec<Effect<E>>,
    /// Local environment to bound the variables of the conditions/effects to values.
    local_env: Env<E>,
}

impl<E: Interpreter> Action<E> {
    pub fn new(name: String, conditions: Vec<Condition<E>>, effects: Vec<Effect<E>>, local_env: Env<E>) -> Self {
        Self {
            name,
            conditions,
            effects,
            local_env,
        }
    }

    pub fn local_env(&self) -> &Env<E> {
        &self.local_env
    }
}

impl<E: Interpreter> Act<E> for Action<E> {
    fn conditions(&self) -> &Vec<Condition<E>> {
        &self.conditions
    }

    fn applicable(&self, env: &Env<E>) -> Result<bool> {
        // Check the conditions.
        for c in self.conditions() {
            if !c.is_valid(env)? {
                return Ok(false);
            }
        }
        // Check that two effects don't affect the same fluent.
        let mut changes: Vec<Vec<Value>> = vec![];
        for e in self.effects.iter() {
            if let Some((f, _)) = e.changes(env)? {
                if changes.contains(&f) {
                    return Ok(false);
                }
                changes.push(f);
            }
        }
        Ok(true)
    }

    fn apply(&self, env: &Env<E>, s: &State) -> Result<Option<State>> {
        if !self.applicable(env)? {
            return Ok(None);
        }
        let mut new_s = s.clone();
        for e in self.effects.iter() {
            if let Some(s) = e.apply(env, &new_s)? {
                new_s = s;
            }
        }
        Ok(Some(new_s))
    }
}

#[derive(Debug, PartialEq, Eq)]
/// Represents an iterator of actions.
pub struct ActionIter<E: Interpreter>(Vec<Action<E>>);

impl<E: Interpreter> From<Vec<Action<E>>> for ActionIter<E> {
    fn from(a: Vec<Action<E>>) -> Self {
        Self(a)
    }
}

impl<E: Interpreter> ActionIter<E> {
    pub fn iter(&self) -> Iter<'_, Action<E>> {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{effects::EffectKind, value::Value};

    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
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

    fn f(s: &str) -> Vec<MockExpr> {
        vec![MockExpr(s.into())]
    }
    fn v(i: i64) -> MockExpr {
        MockExpr(i.into())
    }
    fn c(b: bool) -> Condition<MockExpr> {
        Condition::from(MockExpr(b.into()))
    }
    fn e(cond: &[bool], fs: &str, val: i64) -> Effect<MockExpr> {
        let conditions = cond.iter().map(|b| c(*b)).collect::<Vec<_>>();
        Effect::new(f(fs), v(val), EffectKind::Assign, conditions)
    }
    fn a(cond: &[bool], effects: Vec<Effect<MockExpr>>) -> Action<MockExpr> {
        let conditions = cond.iter().map(|b| c(*b)).collect::<Vec<_>>();
        Action {
            name: "a".into(),
            conditions,
            effects,
            local_env: Env::default(),
        }
    }

    #[test]
    fn conditions() {
        let a = a(&[true, false], vec![]);
        assert_eq!(a.conditions(), &[c(true), c(false)]);
    }

    #[test]
    fn applicable() -> Result<()> {
        let mut env = Env::<MockExpr>::default();
        env.bound_fluent(vec!["a".into()], 10.into());
        env.bound_fluent(vec!["b".into()], 10.into());

        let eta = e(&[true], "a", 5);
        let efa = e(&[false], "a", 5);
        let etb = e(&[true], "b", 2);
        let efb = e(&[false], "b", 2);
        let effects = vec![eta.clone(), etb.clone(), efa.clone(), efb.clone()];

        for condition in vec![true, false] {
            for e1 in effects.iter() {
                for e2 in effects.iter() {
                    let action = a(&[condition], vec![e1.clone(), e2.clone()]);

                    if !condition || (e1 == e2 && e1.applicable(&env)?) {
                        assert!(!action.applicable(&env)?, "{:?}\n{:?}", e1, e2);
                    } else {
                        assert!(action.applicable(&env)?, "{:?}\n{:?}", e1, e2);
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn apply() -> Result<()> {
        let mut env = Env::<MockExpr>::default();
        env.bound_fluent(vec!["a".into()], 10.into());
        env.bound_fluent(vec!["b".into()], 10.into());

        let eta = e(&[true], "a", 5);
        let efa = e(&[false], "a", 5);
        let etb = e(&[true], "b", 2);
        let efb = e(&[false], "b", 2);
        let effects = vec![eta.clone(), etb.clone(), efa.clone(), efb.clone()];

        for condition in vec![true, false] {
            for e1 in effects.iter() {
                for e2 in effects.iter() {
                    let action = a(&[condition], vec![e1.clone(), e2.clone()]);
                    let state = action.apply(&env, env.state())?;

                    if !condition || (e1 == e2 && e1.applicable(&env)?) {
                        assert!(state.is_none(), "{:?}\n{:?}", e1, e2);
                    } else {
                        assert!(state.is_some(), "{:?}\n{:?}", e1, e2);
                        let state = state.unwrap();

                        if *e1 == eta || *e2 == eta {
                            assert_eq!(*state.get(&vec!["a".into()]).unwrap(), 5.into());
                        } else if *e1 == efa || *e2 == efa {
                            assert_eq!(*state.get(&vec!["a".into()]).unwrap(), 10.into());
                        } else if *e1 == etb || *e2 == etb {
                            assert_eq!(*state.get(&vec!["b".into()]).unwrap(), 2.into());
                        } else {
                            // efb
                            assert_eq!(*state.get(&vec!["b".into()]).unwrap(), 10.into())
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
