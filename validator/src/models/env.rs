use std::fmt::Debug;

use im::HashMap;

use crate::{print_assign, procedures::Procedure};

use super::{state::State, value::Value};

/// Represents the current environment of the validation.
#[derive(Default)]
pub struct Env<E> {
    /// Whether or not debug information should be printed.
    pub verbose: bool,
    /// The current state of the world during the validation.
    state: State,
    /// Mapping from a parameter or variable name to its current value.
    vars: HashMap<String, Value>,
    /// Mapping from a function symbol to its actual implementation.
    procedures: HashMap<String, Procedure<E>>,
    /// List of the objects grouped by type.
    objects: HashMap<String, Vec<Value>>,
    /// Hierarchy of the object types.
    types: HashMap<String, Vec<String>>,
}

impl<E> Clone for Env<E> {
    fn clone(&self) -> Self {
        Self {
            verbose: self.verbose,
            state: self.state.clone(),
            vars: self.vars.clone(),
            procedures: self.procedures.clone(),
            objects: self.objects.clone(),
            types: self.types.clone(),
        }
    }
}

impl<E> Debug for Env<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("verbose", &self.verbose)
            .field("state", &self.state)
            .field("vars", &self.vars)
            .field("procedures", &self.procedures.keys().collect::<Vec<_>>())
            .field("objects", &self.objects)
            .field("types", &self.types)
            .finish()
    }
}

impl<E> PartialEq for Env<E> {
    fn eq(&self, other: &Self) -> bool {
        self.verbose == other.verbose
            && self.state == other.state
            && self.vars == other.vars
            && self.procedures.len() == other.procedures.len()
            && self.procedures.keys().all(|k| other.procedures.contains_key(k))
            && self.objects == other.objects
            && self.types == other.types
    }
}

impl<E> Eq for Env<E> {}

impl<E> Env<E> {
    /// Bounds a parameter or a variable to a value.
    pub fn bound(&mut self, t: String, n: String, v: Value) -> Option<Value> {
        let r = self.vars.insert(n, v.clone());

        let new_vec = self
            .objects
            .remove(&t)
            .map(|mut a| {
                a.push(v.clone());
                a
            })
            .unwrap_or_else(|| vec![v])
            .to_vec();
        self.objects.insert(t, new_vec);
        r
    }

    /// Bounds a fluent to a value.
    pub fn bound_fluent(&mut self, k: Vec<Value>, v: Value) -> Option<Value> {
        print_assign!(self.verbose, "{:?} <-- \x1b[1m{:?}\x1b[0m", k, v);
        self.state.bound(k, v)
    }

    /// Bounds a function symbol to a procedure.
    pub fn bound_procedure(&mut self, k: String, v: Procedure<E>) -> Option<Procedure<E>> {
        self.procedures.insert(k, v)
    }

    /// Bounds a type to its parent.
    pub fn bound_type(&mut self, t: String, p: String) -> Option<Vec<String>> {
        let old_vec = self.types.remove(&p);
        let new_vec = old_vec
            .clone()
            .map(|mut v| {
                v.push(t.clone());
                v
            })
            .unwrap_or_else(|| vec![t])
            .to_vec();
        self.types.insert(p, new_vec);
        old_vec
    }

    /// Creates a clone of this environment extended with another.
    pub fn extends_with(&self, e: &Self) -> Self {
        let mut r = self.clone();
        for (k, v) in e.vars.clone() {
            r.vars = r.vars.update(k, v);
        }
        for (k, v) in e.objects.clone() {
            let d = Vec::<Value>::new();
            let mut vector = r.objects.get(&k).unwrap_or(&d).clone();
            vector.extend(v);
            r.objects = r.objects.update(k, vector.to_vec());
        }
        r
    }

    /// Returns a reference to the value corresponding to the parameter or the variable.
    pub fn get(&self, k: &String) -> Option<&Value> {
        self.vars.get(k)
    }

    /// Returns a reference to the value corresponding to the fluent.
    pub fn get_fluent(&self, k: &Vec<Value>) -> Option<&Value> {
        self.state.get(k)
    }

    /// Returns a list of objects with the type.
    pub fn get_objects(&self, t: &String) -> Option<Vec<Value>> {
        let mut r = self.objects.get(t).cloned().unwrap_or_default();
        for tpe in self.types.get(t).cloned().unwrap_or_default() {
            r.extend(self.get_objects(&tpe).unwrap_or_default());
        }

        if r.is_empty() {
            None
        } else {
            Some(r)
        }
    }

    /// Returns a reference to the procedure corresponding to the function symbol.
    pub fn get_procedure(&self, k: &String) -> Option<&Procedure<E>> {
        self.procedures.get(k)
    }

    /// Updates the current state.
    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }

    /// Returns the current state.
    pub fn state(&self) -> &State {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use im::hashmap;

    use super::*;

    fn p(s: &str) -> String {
        s.to_string()
    }
    fn f(s: &str) -> Vec<Value> {
        vec![s.into()]
    }
    fn v(b: bool) -> Value {
        b.into()
    }
    fn proc() -> Procedure<Mock> {
        fn f(_env: &Env<Mock>, _args: Vec<Mock>) -> Result<Value> {
            Ok(v(true))
        }
        f
    }

    #[derive(Default)]
    struct Mock();

    #[test]
    fn default() {
        let e = Env::<Mock>::default();
        assert!(!e.verbose);
        assert_eq!(e.state, State::default());
        assert!(e.vars.is_empty());
        assert!(e.procedures.is_empty());
        assert!(e.objects.is_empty());
    }

    #[test]
    fn eq() {
        let mut e1 = Env::<Mock>::default();
        e1.bound_procedure(p("s"), proc());
        let e2 = Env::<Mock>::default();
        assert_eq!(e1, e1);
        assert_ne!(e1, e2);
        assert_ne!(e2, e1);
        assert_eq!(e2, e2);
    }

    #[test]
    fn bound() {
        let expected_vars = hashmap! {p("s")=>v(true), p("a")=>v(false)};
        let expected_objects = hashmap! {p("t") => vec![v(true), v(false)]};
        let mut e = Env::<Mock>::default();
        e.bound(p("t"), p("s"), v(true));
        e.bound(p("t"), p("a"), v(false));
        assert_eq!(e.vars, expected_vars);
        assert_eq!(e.objects, expected_objects);

        e.vars.clear();
        e.objects.clear();
        assert_eq!(e, Env::default());
    }

    #[test]
    fn bound_fluent() {
        let mut s = State::default();
        s.bound(f("s"), v(true));
        let mut e = Env::<Mock>::default();
        e.bound_fluent(f("s"), v(true));
        assert_eq!(e.state, s);

        e.state = State::default();
        assert_eq!(e, Env::default());
    }

    #[test]
    fn bound_procedure() {
        let mut e = Env::<Mock>::default();
        e.bound_procedure(p("s"), proc());
        assert_eq!(e.procedures.len(), 1);
        assert!(e.procedures.contains_key(&p("s")));
        assert_eq!(e.procedures.get(&p("s")).unwrap()(&e, vec![]).unwrap(), v(true));

        e.procedures.clear();
        assert_eq!(e, Env::default());
    }

    #[test]
    fn bound_type() {
        let expected_types = hashmap! {p("m") => vec![p("a"), p("b")]};
        let mut e = Env::<Mock>::default();
        e.bound_type(p("a"), p("m"));
        e.bound_type(p("b"), p("m"));
        assert_eq!(e.types, expected_types);

        e.types.clear();
        assert_eq!(e, Env::default());
    }

    #[test]
    fn extends_with() {
        let mut e1 = Env::<Mock>::default();
        e1.bound("t".into(), "a".into(), 1.into());
        e1.bound("t".into(), "b".into(), 1.into());
        let mut e2 = Env::<Mock>::default();
        e2.bound("t".into(), "b".into(), 2.into());
        e2.bound("t".into(), "c".into(), 2.into());
        let e3 = e1.extends_with(&e2);

        assert_eq!(*e3.get(&"a".into()).unwrap(), 1.into());
        assert_eq!(*e3.get(&"b".into()).unwrap(), 2.into());
        assert_eq!(*e3.get(&"c".into()).unwrap(), 2.into());
    }

    #[test]
    fn get() {
        let mut e = Env::<Mock>::default();
        e.bound(p("t"), p("s"), v(true));
        assert_eq!(e.get(&p("s")), Some(&v(true)));
        assert_eq!(e.get(&p("a")), None);
    }

    #[test]
    fn get_fluent() {
        let mut e = Env::<Mock>::default();
        e.bound_fluent(f("s"), v(true));
        assert_eq!(e.get_fluent(&f("s")), Some(&v(true)));
        assert_eq!(e.get_fluent(&f("a")), None);
    }

    #[test]
    fn get_objects() {
        let expected_objects = vec![v(true), v(false)];
        let mut e = Env::<Mock>::default();
        e.bound(p("t"), p("s"), v(true));
        e.bound(p("t"), p("a"), v(false));
        assert_eq!(e.get_objects(&"t".into()), Some(expected_objects));
        assert_eq!(e.get_objects(&"a".into()), None);
    }

    #[test]
    fn get_objects_hierarchy() {
        let expected_objects = vec![v(true), v(false)];
        let mut e = Env::<Mock>::default();
        e.bound(p("t"), p("s"), v(true));
        e.bound(p("t"), p("a"), v(false));
        e.bound_type(p("t"), p("m"));
        assert_eq!(e.get_objects(&"m".into()), Some(expected_objects));
        assert_eq!(e.get_objects(&"a".into()), None);
    }

    #[test]
    fn get_procedure() {
        let mut e = Env::<Mock>::default();
        e.bound_procedure(p("s"), proc());
        assert!(e.get_procedure(&p("s")).is_some());
        assert_eq!(e.get_procedure(&p("s")).unwrap()(&e, vec![]).unwrap(), v(true));
        assert!(e.get_procedure(&p("a")).is_none());
    }

    #[test]
    fn set_state() {
        let mut s = State::default();
        s.bound(f("s"), v(true));
        let mut e = Env::<Mock>::default();
        e.set_state(s.clone());
        assert_eq!(e.state, s);

        e.state = State::default();
        assert_eq!(e, Env::default());
    }

    #[test]
    fn state() {
        let mut e = Env::<Mock>::default();
        e.bound_fluent(f("s"), v(true));
        assert_eq!(e.state, *e.state());
    }
}
