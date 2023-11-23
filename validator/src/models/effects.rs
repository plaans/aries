use std::fmt::Display;

use anyhow::{Context, Result};

use crate::{
    print_assign,
    traits::{act::Act, durative::Durative, interpreter::Interpreter},
};

use super::{condition::SpanCondition, env::Env, state::State, time::Timepoint, value::Value};

/* ========================================================================== */
/*                                Effect Kinds                                */
/* ========================================================================== */

#[derive(Clone, Debug, PartialEq, Eq)]
/// Different kinds of effects.
pub enum EffectKind {
    Assign,
    Increase,
    Decrease,
}

/* ========================================================================== */
/*                                 Span Effect                                */
/* ========================================================================== */

#[derive(Clone, Debug, PartialEq, Eq)]
/// Represents an effect of a SpanAction.
pub struct SpanEffect<E> {
    /// The fluent updated by the effect.
    fluent: Vec<E>,
    /// The value used by the effect.
    value: E,
    /// The kind of effect it is.
    kind: EffectKind,
    /// The list of conditions to apply the effects.
    conditions: Vec<SpanCondition<E>>,
}

impl<E> SpanEffect<E> {
    pub fn new(fluent: Vec<E>, value: E, kind: EffectKind, conditions: Vec<SpanCondition<E>>) -> Self {
        Self {
            fluent,
            value,
            kind,
            conditions,
        }
    }

    pub fn kind(&self) -> &EffectKind {
        &self.kind
    }

    /// Returns the optional changes made by this effect.
    ///
    /// # Attributes
    ///
    /// * `env` - A reference to the environment to use to calculate the changes.
    /// * `init_env` - A reference to the environment to use to check the applicability.
    pub fn changes(&self, env: &Env<E>, init_env: &Env<E>) -> Result<Option<(Vec<Value>, Value)>>
    where
        E: Interpreter,
    {
        if !self.applicable(init_env)? {
            return Ok(None);
        }
        let f = self.fluent.iter().map(|e| e.eval(env)).collect::<Result<Vec<_>>>()?;
        let v = self.value.eval(env)?;
        let nv = match self.kind {
            EffectKind::Assign => v,
            EffectKind::Increase => (env.get_fluent(&f).context(format!("Unbounded fluent {f:?}"))?.clone() + v)?,
            EffectKind::Decrease => (env.get_fluent(&f).context(format!("Unbounded fluent {f:?}"))?.clone() - v)?,
        };
        Ok(Some((f, nv)))
    }
}

impl<E: Interpreter> Act<E> for SpanEffect<E> {
    fn conditions(&self) -> &Vec<SpanCondition<E>> {
        &self.conditions
    }

    fn apply(&self, env: &Env<E>, init_env: &Env<E>) -> Result<Option<State>> {
        if let Some((f, nv)) = self.changes(env, init_env)? {
            print_assign!(env.verbose, "{:?} <-- \x1b[1m{:?}\x1b[0m", f, nv);
            let mut s = env.state().clone();
            s.bound(f, nv)?;
            Ok(Some(s))
        } else {
            Ok(None)
        }
    }
}

impl<E: Display> Display for SpanEffect<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fl = self.fluent.iter().map(|f| format!("{f}")).collect::<Vec<_>>().join(" ");
        match self.kind {
            EffectKind::Assign => f.write_fmt(format_args!("{} <- {}", fl, self.value)),
            EffectKind::Increase => f.write_fmt(format_args!("{} += {}", fl, self.value)),
            EffectKind::Decrease => f.write_fmt(format_args!("{} -= {}", fl, self.value)),
        }
    }
}

/* ========================================================================== */
/*                               Durative Effect                              */
/* ========================================================================== */

#[derive(Clone, Debug, PartialEq, Eq)]
/// Represents an effect of a DurativeAction.
pub struct DurativeEffect<E> {
    /// The span effect associated to this durative one.
    span: SpanEffect<E>,
    /// The timepoint where effect must occurred.
    occurrence: Timepoint,
}

impl<E> DurativeEffect<E> {
    pub fn new(
        fluent: Vec<E>,
        value: E,
        kind: EffectKind,
        conditions: Vec<SpanCondition<E>>,
        occurrence: Timepoint,
    ) -> Self {
        Self {
            span: SpanEffect {
                fluent,
                value,
                kind,
                conditions,
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

impl<E> Durative<E> for DurativeEffect<E> {
    fn start(&self, _: &Env<E>) -> &Timepoint {
        self.occurrence()
    }

    fn end(&self, _: &Env<E>) -> &Timepoint {
        self.occurrence()
    }

    fn is_start_open(&self) -> bool {
        false
    }

    fn is_end_open(&self) -> bool {
        false
    }
}

impl<E: Display> Display for DurativeEffect<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("at {} {}", self.occurrence, self.span))
    }
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

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

        fn convert_to_csp_constraint(&self, _: &Env<Self>) -> Result<crate::models::csp::CspConstraint> {
            todo!()
        }
    }

    fn f(s: &str) -> Vec<MockExpr> {
        vec![MockExpr(s.into())]
    }
    fn v(i: i64) -> MockExpr {
        MockExpr(i.into())
    }
    fn c(b: bool) -> SpanCondition<MockExpr> {
        SpanCondition::new(MockExpr(b.into()))
    }
    fn e(cond: &[bool]) -> SpanEffect<MockExpr> {
        let conditions = cond.iter().map(|b| c(*b)).collect::<Vec<_>>();
        SpanEffect::new(f("s"), v(1), EffectKind::Assign, conditions)
    }

    #[test]
    fn conditions() {
        let e = e(&[true, false]);
        assert_eq!(e.conditions(), &[c(true), c(false)]);
    }

    #[test]
    fn changes() -> Result<()> {
        let mut env = Env::<MockExpr>::default();
        env.bound_fluent(vec!["s".into()], 10.into())?;
        let a = SpanEffect::new(f("s"), v(1), EffectKind::Assign, vec![]);
        let i = SpanEffect::new(f("s"), v(1), EffectKind::Increase, vec![]);
        let d = SpanEffect::new(f("s"), v(1), EffectKind::Decrease, vec![]);
        let f = e(&[false]);

        assert_eq!(a.changes(&env, &env)?, Some((vec!["s".into()], 1.into())));
        assert_eq!(i.changes(&env, &env)?, Some((vec!["s".into()], 11.into())));
        assert_eq!(d.changes(&env, &env)?, Some((vec!["s".into()], 9.into())));
        assert_eq!(f.changes(&env, &env)?, None);
        Ok(())
    }

    #[test]
    fn apply() -> Result<()> {
        let mut env = Env::<MockExpr>::default();
        env.bound_fluent(vec!["s".into()], 10.into())?;
        let a = SpanEffect::new(f("s"), v(1), EffectKind::Assign, vec![]);
        let i = SpanEffect::new(f("s"), v(1), EffectKind::Increase, vec![]);
        let d = SpanEffect::new(f("s"), v(1), EffectKind::Decrease, vec![]);
        let f = e(&[false]);

        let mut sa = State::default();
        sa.bound(vec!["s".into()], 1.into())?;
        let mut si = State::default();
        si.bound(vec!["s".into()], 11.into())?;
        let mut sd = State::default();
        sd.bound(vec!["s".into()], 9.into())?;

        assert_eq!(a.apply(&env, &env)?, Some(sa));
        assert_eq!(i.apply(&env, &env)?, Some(si));
        assert_eq!(d.apply(&env, &env)?, Some(sd));
        assert_eq!(f.apply(&env, &env)?, None);

        Ok(())
    }

    #[test]
    fn apply_out_bounds() -> Result<()> {
        let n = "s -- integer[10, 10]";
        let mut env = Env::<MockExpr>::default();
        env.bound_fluent(vec![n.into()], 10.into())?;
        let a = SpanEffect::new(f(n), v(1), EffectKind::Assign, vec![]);
        let i = SpanEffect::new(f(n), v(1), EffectKind::Increase, vec![]);
        let d = SpanEffect::new(f(n), v(1), EffectKind::Decrease, vec![]);

        // NOTE - Applying an effect does not check that the new value is inside the bounds. See `State::check_bounds()` for that.
        assert!(a.apply(&env, &env).is_ok());
        assert!(i.apply(&env, &env).is_ok());
        assert!(d.apply(&env, &env).is_ok());
        Ok(())
    }
}
