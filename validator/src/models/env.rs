use std::{collections::HashMap, fmt::Debug};

use anyhow::{Context, Result};

use crate::{interfaces::unified_planning::constants::*, print_assign, procedures::*};

use super::{expression::ValExpression, state::State, value::Value};

type Procedure = fn(&Env, &[Box<dyn ValExpression>]) -> Result<Value>;

/// Represents the current environment of the validation.
#[derive(Clone, Eq)]
pub struct Env {
    /// Whether or not debug information should be printed.
    pub verbose: bool,
    /// The current state of the world during the validation.
    state: State,
    /// Mapping from a parameter or variable name to its current value.
    vars: HashMap<String, Value>,
    /// Mapping from a function symbol to its actual implementation.
    procedures: HashMap<String, Procedure>,
    /// List of objects grouped by type.
    objects: HashMap<String, Vec<Value>>,
    /// Default values of the fluents
    fluent_defaults: HashMap<String, Value>,
}

impl Debug for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("verbose", &self.verbose)
            .field("state", &self.state)
            .field("vars", &self.vars)
            .field("procedures", &self.procedures.keys())
            .field("objects", &self.objects)
            .field("fluent_defaults", &self.fluent_defaults)
            .finish()
    }
}

impl Default for Env {
    fn default() -> Self {
        let procedures: HashMap<String, Procedure> = HashMap::from([
            (UP_AND.to_string(), and as Procedure),
            (UP_OR.to_string(), or as Procedure),
            (UP_NOT.to_string(), not as Procedure),
            (UP_IMPLIES.to_string(), implies as Procedure),
            (UP_EQUALS.to_string(), equals as Procedure),
            (UP_LE.to_string(), le as Procedure),
            (UP_PLUS.to_string(), plus as Procedure),
            (UP_MINUS.to_string(), minus as Procedure),
            (UP_TIMES.to_string(), times as Procedure),
            (UP_DIV.to_string(), div as Procedure),
            (UP_EXISTS.to_string(), exists as Procedure),
            (UP_FORALL.to_string(), forall as Procedure),
        ]);

        Self {
            verbose: false,
            state: Default::default(),
            vars: Default::default(),
            procedures,
            objects: Default::default(),
            fluent_defaults: Default::default(),
        }
    }
}

impl PartialEq for Env {
    fn eq(&self, other: &Self) -> bool {
        let mut result = self.verbose == other.verbose
            && self.state == other.state
            && self.vars == other.vars
            && self.procedures.len() == other.procedures.len()
            && self.objects == other.objects
            && self.fluent_defaults == other.fluent_defaults;
        for key in self.procedures.keys() {
            result &= other.procedures.contains_key(key);
        }
        result
    }
}

impl Env {
    /// Adds a default value to the given fluent.
    pub fn add_default_fluent(&mut self, s: String, v: Value) {
        self.fluent_defaults.insert(s, v);
    }

    /// Adds a new procedure to the environment.
    pub fn add_procedure(&mut self, s: String, p: Procedure) {
        self.procedures.insert(s, p);
    }

    /// Bounds a parameter or variable name to its current value.
    pub fn bound(&mut self, tpe: String, name: String, value: Value) {
        self.vars.insert(name, value.clone());

        let new_vec: Vec<Value> = self
            .objects
            .remove(&tpe)
            .map(|mut v| {
                v.push(value.clone());
                v
            })
            .unwrap_or_else(|| vec![value])
            .to_vec();
        self.objects.insert(tpe, new_vec);
    }

    /// Bounds a fluent to its current value.
    pub fn bound_fluent(&mut self, fluent: Vec<Value>, value: Value) {
        print_assign!(self.verbose, "{:?} <-- \x1b[1m{:?}\x1b[0m", fluent, value);
        self.state.bound(fluent, value);
    }

    /// Changes the current state of the validation.
    pub fn update_state(&mut self, s: State) {
        self.state = s;
    }

    /// Returns the fluent, evaluated with the given arguments, in the current state.
    ///
    /// If the fluent doesn't exists, returns its default value.
    pub fn get_fluent(&self, f: &String, args: &[Box<dyn ValExpression>]) -> Result<Value> {
        let fluent =
            args.iter()
                .fold::<Result<_>, _>(Ok(Vec::<Value>::from([Value::Symbol(f.into())])), |acc, arg| {
                    let mut new_acc = acc?.to_vec();
                    new_acc.push(arg.eval(self)?);
                    Ok(new_acc)
                })?;
        self.state.get_fluent(&fluent).or_else(|_| {
            self.fluent_defaults
                .get(f)
                .context(format!("No default value for the fluent {:?}", f))
                .cloned()
        })
    }

    /// Returns the objects of the given type.
    pub fn get_objects(&self, t: &String) -> Result<Vec<Value>> {
        self.objects
            .get(t)
            .context(format!("No objects of type {:?}", t))
            .cloned()
    }

    /// Returns the implementation of the given function symbol.
    pub fn get_procedure(&self, s: &String) -> Result<Procedure> {
        self.procedures
            .get(s)
            .context(format!("No procedure called {:?}", s))
            .cloned()
    }

    /// Returns the current state.
    pub fn get_state(&self) -> State {
        self.state.clone()
    }

    /// Returns the current value of the given parameter or variable name.
    pub fn get_var(&self, s: &String) -> Result<Value> {
        self.vars.get(s).context(format!("Unbounded variable {:?}", s)).cloned()
    }
}

#[cfg(test)]
mod tests {
    use unified_planning::{atom::Content, Atom, Expression, ExpressionKind};

    use super::*;

    #[test]
    fn default() -> Result<()> {
        let env = Env::default();
        assert_eq!(env.verbose, false);
        assert_eq!(env.state, State::default());
        assert!(env.vars.is_empty());
        assert_eq!(env.procedures.len(), 12);
        assert!(env.procedures.contains_key(UP_AND));
        assert!(env.procedures.contains_key(UP_OR));
        assert!(env.procedures.contains_key(UP_NOT));
        assert!(env.procedures.contains_key(UP_IMPLIES));
        assert!(env.procedures.contains_key(UP_EQUALS));
        assert!(env.procedures.contains_key(UP_LE));
        assert!(env.procedures.contains_key(UP_PLUS));
        assert!(env.procedures.contains_key(UP_MINUS));
        assert!(env.procedures.contains_key(UP_TIMES));
        assert!(env.procedures.contains_key(UP_DIV));
        assert!(env.procedures.contains_key(UP_EXISTS));
        assert!(env.procedures.contains_key(UP_FORALL));
        assert!(env.objects.is_empty());
        assert!(env.fluent_defaults.is_empty());
        Ok(())
    }

    #[test]
    fn add_procedure() -> Result<()> {
        fn proc(_env: &Env, _args: &[Box<dyn ValExpression>]) -> Result<Value> {
            Ok(Value::Bool(true))
        }

        let mut env = Env::default();
        env.add_procedure("s".into(), proc);
        assert_eq!(env.procedures.len(), Env::default().procedures.len() + 1);
        assert_eq!(env.get_procedure(&"s".into())?(&env, &vec![])?, Value::Bool(true));

        env.procedures.remove("s".into());
        assert_eq!(env, Env::default());
        Ok(())
    }

    #[test]
    fn add_default_fluent() -> Result<()> {
        let mut env = Env::default();
        env.add_default_fluent("f".into(), Value::Bool(true));
        assert_eq!(
            env.fluent_defaults,
            HashMap::<String, Value>::from([("f".into(), Value::Bool(true))])
        );

        env.fluent_defaults.clear();
        assert_eq!(env, Env::default());
        Ok(())
    }

    #[test]
    fn bound() -> Result<()> {
        let mut env = Env::default();
        env.bound("t".into(), "s".into(), Value::Bool(true));
        env.bound("t".into(), "k".into(), Value::Bool(false));
        assert_eq!(
            env.vars,
            HashMap::<String, Value>::from([("s".into(), Value::Bool(true)), ("k".into(), Value::Bool(false))])
        );
        assert_eq!(
            env.objects,
            HashMap::<String, Vec<Value>>::from([("t".into(), vec![Value::Bool(true), Value::Bool(false)])])
        );

        env.vars.clear();
        env.objects.clear();
        assert_eq!(env, Env::default());
        Ok(())
    }

    #[test]
    fn bound_fluent() -> Result<()> {
        let mut env = Env::default();
        env.bound_fluent(vec![Value::Symbol("s".into())], Value::Bool(true));
        let mut state = State::default();
        state.bound(vec![Value::Symbol("s".into())], Value::Bool(true));
        assert_eq!(env.state, state);

        env.state = State::default();
        assert_eq!(env, Env::default());
        Ok(())
    }

    #[test]
    fn update_state() -> Result<()> {
        let mut env = Env::default();
        let mut state = State::default();
        state.bound(vec![Value::Symbol("s".into())], Value::Bool(true));
        env.update_state(state.clone());
        assert_eq!(env.state, state);

        env.state = State::default();
        assert_eq!(env, Env::default());
        Ok(())
    }

    #[test]
    fn get_fluent() -> Result<()> {
        let mut state = State::default();
        state.bound(
            vec![Value::Symbol("loc".into()), Value::Symbol("R1".into())],
            Value::Symbol("L3".into()),
        );
        let mut env = Env::default();
        env.bound("r".into(), "R1".into(), Value::Symbol("R1".into()));
        env.update_state(state);
        let value = env.get_fluent(
            &"loc".into(),
            &vec![Box::new(Expression {
                atom: Some(Atom {
                    content: Some(Content::Symbol("R1".into())),
                }),
                kind: ExpressionKind::Parameter.into(),
                ..Default::default()
            }) as Box<dyn ValExpression>],
        )?;
        assert_eq!(value, Value::Symbol("L3".into()));
        Ok(())
    }

    #[test]
    fn get_fluent_default() -> Result<()> {
        let mut env = Env::default();
        env.add_default_fluent("loc".into(), Value::Symbol("L3".into()));
        env.bound("r".into(), "R1".into(), Value::Symbol("R1".into()));
        let good_value = env.get_fluent(
            &"loc".into(),
            &vec![Box::new(Expression {
                atom: Some(Atom {
                    content: Some(Content::Symbol("R1".into())),
                }),
                kind: ExpressionKind::Parameter.into(),
                ..Default::default()
            }) as Box<dyn ValExpression>],
        );
        let bad_value = env.get_fluent(
            &"pos".into(),
            &vec![Box::new(Expression {
                atom: Some(Atom {
                    content: Some(Content::Symbol("R1".into())),
                }),
                kind: ExpressionKind::Parameter.into(),
                ..Default::default()
            }) as Box<dyn ValExpression>],
        );
        assert_eq!(good_value?, Value::Symbol("L3".into()));
        assert!(bad_value.is_err());
        Ok(())
    }

    #[test]
    fn get_objects() -> Result<()> {
        let mut env = Env::default();
        env.bound("t".into(), "s".into(), Value::Bool(true));
        env.bound("t".into(), "k".into(), Value::Bool(false));
        let expected_objects = vec![Value::Bool(true), Value::Bool(false)];
        assert_eq!(env.get_objects(&"t".into())?, expected_objects);
        Ok(())
    }

    #[test]
    fn get_procedure() -> Result<()> {
        let mut env = Env::default();
        env.add_procedure("s".into(), |_, _| Ok(Value::Bool(true)));
        assert_eq!(env.get_procedure(&"s".into())?(&env, &vec![])?, Value::Bool(true));
        assert!(env.get_procedure(&"a".into()).is_err());
        Ok(())
    }

    #[test]
    fn get_var() -> Result<()> {
        let mut env = Env::default();
        env.bound("r".into(), "R1".into(), Value::Symbol("R1".into()));
        assert_eq!(env.get_var(&"R1".into())?, Value::Symbol("R1".into()));
        assert!(env.get_var(&"R0".into()).is_err());
        Ok(())
    }
}
