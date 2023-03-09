use anyhow::Result;

use crate::traits::{durative::Durative, interpreter::Interpreter};

use super::{
    env::Env,
    time::{TemporalInterval, Timepoint},
};

/* ========================================================================== */
/*                            Condition Enumeration                           */
/* ========================================================================== */

/// Represents a span or durative condition.
#[derive(Clone, Debug)]
pub enum Condition<E: Interpreter> {
    Span(SpanCondition<E>),
    Durative(DurativeCondition<E>),
}

/* ========================================================================== */
/*                               Span Condition                               */
/* ========================================================================== */

#[derive(Clone, Debug, PartialEq, Eq)]
/// Represents a condition of a SpanAction.
pub struct SpanCondition<E> {
    /// The expression of the condition.
    expr: E,
}

impl<E> SpanCondition<E> {
    pub fn new(expr: E) -> Self {
        Self { expr }
    }

    /// Whether or not the condition is valid in the environment.
    pub fn is_valid(&self, env: &Env<E>) -> Result<bool>
    where
        E: Interpreter,
    {
        Ok(self.expr().eval(env)? == true.into())
    }

    /// Returns the expression of the condition.
    pub fn expr(&self) -> &E {
        &self.expr
    }
}

/* ========================================================================== */
/*                             Durative Condition                             */
/* ========================================================================== */

#[derive(Clone, Debug, PartialEq, Eq)]
/// Represents a condition of a DurativeAction.
pub struct DurativeCondition<E> {
    /// The span condition associated to this durative one.
    span: SpanCondition<E>,
    /// The time interval where the condition must be verified.
    interval: TemporalInterval,
}

impl<E> DurativeCondition<E> {
    pub fn new(expr: E, interval: TemporalInterval) -> Self {
        Self {
            span: SpanCondition { expr },
            interval,
        }
    }

    /// Creates a new durative condition from a span one.
    pub fn from_span(span: SpanCondition<E>, interval: TemporalInterval) -> Self {
        Self { span, interval }
    }

    /// Returns the condition as a span one.
    pub fn to_span(&self) -> &SpanCondition<E> {
        &self.span
    }

    /// Returns the expression of the condition.
    pub fn expr(&self) -> &E {
        self.span.expr()
    }

    /// Returns the time interval where the condition must be verified.
    pub fn interval(&self) -> &TemporalInterval {
        &self.interval
    }
}

impl<E> Durative<E> for DurativeCondition<E> {
    fn start(&self, _: &Env<E>) -> &Timepoint {
        self.interval.start()
    }

    fn end(&self, _: &Env<E>) -> &Timepoint {
        self.interval.end()
    }

    fn is_start_open(&self) -> bool {
        self.interval.is_start_open()
    }

    fn is_end_open(&self) -> bool {
        self.interval.is_end_open()
    }
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod span_tests {
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
        let t = SpanCondition::new(MockExpr(true.into()));
        let f = SpanCondition::new(MockExpr(false.into()));

        assert!(t.is_valid(&env)?);
        assert!(!f.is_valid(&env)?);
        Ok(())
    }
}
