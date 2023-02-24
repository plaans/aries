use anyhow::{Context, Result};

use crate::{
    print_assign,
    traits::{act::Act, interpreter::Interpreter},
};

use super::{condition::SpanCondition, env::Env, state::State, time::Timepoint, value::Value};

/*******************************************************************/

#[derive(Clone, Debug, PartialEq, Eq)]
/// Different kinds of effects.
pub enum EffectKind {
    Assign,
    Increase,
    Decrease,
}

/*******************************************************************/

#[derive(Clone, Debug, PartialEq, Eq)]
/// Represents an effect of a SpanAction.
pub struct SpanEffect<E: Interpreter> {
    /// The fluent updated by the effect.
    fluent: Vec<E>,
    /// The value used by the effect.
    value: E,
    /// The kind of effect it is.
    kind: EffectKind,
    /// The list of conditions to apply the effects.
    conditions: Vec<SpanCondition<E>>,
    /// Mapping to bound the variables to values.
    param_bounding: Vec<(String, String, Value)>,
}

impl<E: Interpreter> SpanEffect<E> {
    pub fn new(
        fluent: Vec<E>,
        value: E,
        kind: EffectKind,
        conditions: Vec<SpanCondition<E>>,
        param_bounding: Vec<(String, String, Value)>,
    ) -> Self {
        Self {
            fluent,
            value,
            kind,
            conditions,
            param_bounding,
        }
    }

    /// Returns the optional changes made by this effect.
    pub fn changes(&self, env: &Env<E>) -> Result<Option<(Vec<Value>, Value)>> {
        let mut new_env = env.clone();
        for (t, n, v) in self.param_bounding.iter() {
            new_env.bound(t.clone(), n.clone(), v.clone());
        }

        if !self.applicable(&new_env)? {
            return Ok(None);
        }
        let f = self
            .fluent
            .iter()
            .map(|e| e.eval(&new_env))
            .collect::<Result<Vec<_>>>()?;
        let v = self.value.eval(&new_env)?;
        let nv = match self.kind {
            EffectKind::Assign => v,
            EffectKind::Increase => {
                let cv = new_env
                    .get_fluent(&f)
                    .context(format!("Unbounded fluent {f:?}"))?
                    .clone();
                (cv + v)?
            }
            EffectKind::Decrease => {
                let cv = new_env
                    .get_fluent(&f)
                    .context(format!("Unbounded fluent {f:?}"))?
                    .clone();
                (cv - v)?
            }
        };
        Ok(Some((f, nv)))
    }
}

impl<E: Interpreter> Act<E> for SpanEffect<E> {
    fn conditions(&self) -> &Vec<SpanCondition<E>> {
        &self.conditions
    }

    fn apply(&self, env: &Env<E>, s: &State) -> Result<Option<State>> {
        let mut r = s.clone();
        if let Some((f, nv)) = self.changes(env)? {
            print_assign!(env.verbose, "{:?} <-- \x1b[1m{:?}\x1b[0m", f, nv);
            r.bound(f, nv);
            Ok(Some(r))
        } else {
            Ok(None)
        }
    }
}

/*******************************************************************/

#[derive(Clone, Debug, PartialEq, Eq)]
/// Represents an effect of a DurativeAction.
pub struct DurativeEffect<E: Interpreter> {
    /// The span effect associated to this durative one.
    span: SpanEffect<E>,
    /// The timepoint where effect must occurred.
    occurrence: Timepoint,
}

impl<E: Interpreter> DurativeEffect<E> {
    pub fn new(
        fluent: Vec<E>,
        value: E,
        kind: EffectKind,
        conditions: Vec<SpanCondition<E>>,
        occurrence: Timepoint,
        param_bounding: Vec<(String, String, Value)>,
    ) -> Self {
        Self {
            span: SpanEffect {
                fluent,
                value,
                kind,
                conditions,
                param_bounding,
            },
            occurrence,
        }
    }

    /// Creates a new durative effect from a span one.
    pub fn from_span(span: SpanEffect<E>, occurrence: Timepoint) -> Self {
        Self { span, occurrence }
    }

    /// Returns the effect as a span one.
    pub fn to_span(&self) -> &SpanEffect<E> {
        &self.span
    }

    /// Returns when the effect must occurred.
    pub fn occurrence(&self) -> &Timepoint {
        &self.occurrence
    }
}

/*******************************************************************/

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
    fn c(b: bool) -> SpanCondition<MockExpr> {
        SpanCondition::new(MockExpr(b.into()), vec![])
    }
    fn e(cond: &[bool]) -> SpanEffect<MockExpr> {
        let conditions = cond.iter().map(|b| c(*b)).collect::<Vec<_>>();
        SpanEffect::new(f("s"), v(1), EffectKind::Assign, conditions, vec![])
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
        let a = SpanEffect::new(f("s"), v(1), EffectKind::Assign, vec![], vec![]);
        let i = SpanEffect::new(f("s"), v(1), EffectKind::Increase, vec![], vec![]);
        let d = SpanEffect::new(f("s"), v(1), EffectKind::Decrease, vec![], vec![]);
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
        let a = SpanEffect::new(f("s"), v(1), EffectKind::Assign, vec![], vec![]);
        let i = SpanEffect::new(f("s"), v(1), EffectKind::Increase, vec![], vec![]);
        let d = SpanEffect::new(f("s"), v(1), EffectKind::Decrease, vec![], vec![]);
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
