use anyhow::Result;

use crate::traits::{act::Act, interpreter::Interpreter};

use super::{
    condition::{DurativeCondition, SpanCondition},
    effects::{DurativeEffect, SpanEffect},
    env::Env,
    parameter::Parameter,
    state::State,
    time::Timepoint,
    value::Value,
};

/*******************************************************************/

/// Represents a span or a durative action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action<E: Interpreter> {
    Span(SpanAction<E>),
    Durative(DurativeAction<E>),
}

impl<E: Interpreter> From<SpanAction<E>> for Action<E> {
    fn from(a: SpanAction<E>) -> Self {
        Action::Span(a)
    }
}

impl<E: Interpreter> From<DurativeAction<E>> for Action<E> {
    fn from(a: DurativeAction<E>) -> Self {
        Action::Durative(a)
    }
}

/*******************************************************************/

/// Common parts of a SpanAction and a DurativeAction.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BaseAction {
    /// The name of the action.
    name: String,
    /// The identifier of the action that might be used to refer to it (e.g. in HTN plans).
    id: String,
    /// The parameters of the action.
    params: Vec<Parameter>,
}

/*******************************************************************/

#[derive(Clone, Debug, PartialEq, Eq)]
/// Representation of a span action for the validation.
pub struct SpanAction<E: Interpreter> {
    /// The common parts of a span and a durative action.
    base: BaseAction,
    /// The list of conditions for the application of the action.
    conditions: Vec<SpanCondition<E>>,
    /// The list of effects.
    effects: Vec<SpanEffect<E>>,
}

impl<E: Interpreter> SpanAction<E> {
    pub fn new(
        name: String,
        id: String,
        params: Vec<Parameter>,
        conditions: Vec<SpanCondition<E>>,
        effects: Vec<SpanEffect<E>>,
    ) -> Self {
        Self {
            base: BaseAction { name, id, params },
            conditions,
            effects,
        }
    }

    /// Returns the name of the action.
    pub fn name(&self) -> &String {
        &self.base.name
    }

    /// Returns the id of the action.
    pub fn id(&self) -> &String {
        &self.base.id
    }

    /// Add a new condition to the action.
    pub fn add_condition(&mut self, value: SpanCondition<E>) {
        self.conditions.push(value)
    }

    /// Add a new effect to the action.
    pub fn add_effect(&mut self, value: SpanEffect<E>) {
        self.effects.push(value)
    }
}

impl<E: Interpreter> Act<E> for SpanAction<E> {
    fn conditions(&self) -> &Vec<SpanCondition<E>> {
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
        let mut new_env = env.clone();
        for param in self.base.params.iter() {
            param.bound(&mut new_env);
        }
        if !self.applicable(&new_env)? {
            return Ok(None);
        }
        let mut new_s = s.clone();
        for e in self.effects.iter() {
            if let Some(s) = e.apply(&new_env, &new_s)? {
                new_s = s;
            }
        }
        Ok(Some(new_s))
    }
}

/*******************************************************************/

#[derive(Clone, Debug, PartialEq, Eq)]
/// Representation of  a durative action for the validation.
pub struct DurativeAction<E: Interpreter> {
    /// The common parts of a span and a durative action.
    base: BaseAction,
    /// The list of conditions for the application of the action.
    conditions: Vec<DurativeCondition<E>>,
    /// The list of effects.
    effects: Vec<DurativeEffect<E>>,
    /// The start timepoint of the action.
    start: Timepoint,
    /// The end timepoint of the action.
    end: Timepoint,
}

impl<E: Interpreter> DurativeAction<E> {
    pub fn new(
        name: String,
        id: String,
        params: Vec<Parameter>,
        conditions: Vec<DurativeCondition<E>>,
        effects: Vec<DurativeEffect<E>>,
        start: Timepoint,
        end: Timepoint,
    ) -> Self {
        Self {
            base: BaseAction { name, id, params },
            conditions,
            effects,
            start,
            end,
        }
    }

    /// Returns the parameters of the action.
    pub fn params(&self) -> &[Parameter] {
        self.base.params.as_ref()
    }

    /// Returns the start timepoint of the action.
    pub fn start(&self) -> &Timepoint {
        &self.start
    }

    /// Returns the id of the action.
    pub fn id(&self) -> &String {
        &self.base.id
    }

    /// Returns the end timepoint of the action.
    pub fn end(&self) -> &Timepoint {
        &self.end
    }

    /// Returns the conditions of the action.
    pub fn conditions(&self) -> &[DurativeCondition<E>] {
        self.conditions.as_ref()
    }

    /// Returns the effects of the action.
    pub fn effects(&self) -> &[DurativeEffect<E>] {
        self.effects.as_ref()
    }
}

/*******************************************************************/

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
    fn c(b: bool) -> SpanCondition<MockExpr> {
        SpanCondition::new(MockExpr(b.into()))
    }
    fn e(cond: &[bool], fs: &str, val: i64) -> SpanEffect<MockExpr> {
        let conditions = cond.iter().map(|b| c(*b)).collect::<Vec<_>>();
        SpanEffect::new(f(fs), v(val), EffectKind::Assign, conditions)
    }
    fn sa(cond: &[bool], effects: Vec<SpanEffect<MockExpr>>) -> SpanAction<MockExpr> {
        let conditions = cond.iter().map(|b| c(*b)).collect::<Vec<_>>();
        SpanAction::new("a".into(), "".into(), vec![], conditions, effects)
    }
    fn da() -> DurativeAction<MockExpr> {
        let s = Timepoint::fixed(5.into());
        let e = Timepoint::fixed(10.into());
        DurativeAction::new("d".into(), "".into(), vec![], vec![], vec![], s, e)
    }

    #[test]
    fn from_span() {
        assert_eq!(Action::Span(sa(&[], vec![])), sa(&[], vec![]).into());
    }

    #[test]
    fn from_durative() {
        assert_eq!(Action::Durative(da()), da().into());
    }

    #[test]
    fn conditions() {
        let a = sa(&[true, false], vec![]);
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
                    let conditions = [condition];
                    let action = sa(&conditions, vec![e1.clone(), e2.clone()]);

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
                    let conditions = [condition];
                    let action = sa(&conditions, vec![e1.clone(), e2.clone()]);
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
