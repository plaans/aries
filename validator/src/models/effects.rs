use anyhow::{Context, Result};

use crate::traits::{act::Act, interpreter::Interpreter};

use super::{condition::Condition, env::Env, state::State, value::Value};

#[derive(Clone, Debug, PartialEq, Eq)]
/// Different kinds of effects.
pub enum EffectKind {
    Assign,
    Increase,
    Decrease,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Represents an effect of an Action.
pub struct Effect<E: Interpreter> {
    /// The fluent updated by the effect.
    fluent: Vec<E>,
    /// The value used by the effect.
    value: E,
    /// The kind of effect it is.
    kind: EffectKind,
    /// The list of conditions to apply the effects.
    conditions: Vec<Condition<E>>,
}

impl<E: Interpreter> Effect<E> {
    pub fn new(fluent: Vec<E>, value: E, kind: EffectKind, conditions: Vec<Condition<E>>) -> Self {
        Self {
            fluent,
            value,
            kind,
            conditions,
        }
    }

    /// Returns the optional changes made by this effect.
    pub fn changes(&self, env: &Env<E>) -> Result<Option<(Vec<Value>, Value)>> {
        if !self.applicable(env)? {
            return Ok(None);
        }
        let f = self.fluent.iter().map(|e| e.eval(env)).collect::<Result<Vec<_>>>()?;
        let v = self.value.eval(env)?;
        let nv = match self.kind {
            EffectKind::Assign => v,
            EffectKind::Increase => {
                let cv = env.get_fluent(&f).context(format!("Unbounded fluent {:?}", f))?.clone();
                (cv + v)?
            }
            EffectKind::Decrease => {
                let cv = env.get_fluent(&f).context(format!("Unbounded fluent {:?}", f))?.clone();
                (cv - v)?
            }
        };
        Ok(Some((f, nv)))
    }
}

impl<E: Interpreter> Act<E> for Effect<E> {
    fn conditions(&self) -> &Vec<Condition<E>> {
        &self.conditions
    }

    fn apply(&self, env: &Env<E>, s: &State) -> Result<Option<State>> {
        let mut r = s.clone();
        if let Some((f, nv)) = self.changes(env)? {
            r.bound(f, nv);
            Ok(Some(r))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::models::value::Value;

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
    fn e(cond: &[bool]) -> Effect<MockExpr> {
        let conditions = cond.iter().map(|b| c(*b)).collect::<Vec<_>>();
        Effect::new(f("s"), v(1), EffectKind::Assign, conditions)
    }

    #[test]
    fn conditions() {
        let e = e(&[true, false]);
        assert_eq!(e.conditions(), &[c(true), c(false)]);
    }

    #[test]
    fn changes() -> Result<()> {
        let mut env = Env::<MockExpr>::default();
        env.bound_fluent(vec!["s".into()], 10.into());
        let a = Effect::new(f("s"), v(1), EffectKind::Assign, vec![]);
        let i = Effect::new(f("s"), v(1), EffectKind::Increase, vec![]);
        let d = Effect::new(f("s"), v(1), EffectKind::Decrease, vec![]);
        let f = e(&[false]);

        assert_eq!(a.changes(&env)?, Some((vec!["s".into()], 1.into())));
        assert_eq!(i.changes(&env)?, Some((vec!["s".into()], 11.into())));
        assert_eq!(d.changes(&env)?, Some((vec!["s".into()], 9.into())));
        assert_eq!(f.changes(&env)?, None);
        Ok(())
    }

    #[test]
    fn apply() -> Result<()> {
        let mut env = Env::<MockExpr>::default();
        env.bound_fluent(vec!["s".into()], 10.into());
        let a = Effect::new(f("s"), v(1), EffectKind::Assign, vec![]);
        let i = Effect::new(f("s"), v(1), EffectKind::Increase, vec![]);
        let d = Effect::new(f("s"), v(1), EffectKind::Decrease, vec![]);
        let f = e(&[false]);

        let mut sa = State::default();
        sa.bound(vec!["s".into()], 1.into());
        let mut si = State::default();
        si.bound(vec!["s".into()], 11.into());
        let mut sd = State::default();
        sd.bound(vec!["s".into()], 9.into());

        assert_eq!(a.apply(&env, env.state())?, Some(sa));
        assert_eq!(i.apply(&env, env.state())?, Some(si));
        assert_eq!(d.apply(&env, env.state())?, Some(sd));
        assert_eq!(f.apply(&env, env.state())?, None);

        Ok(())
    }
}
