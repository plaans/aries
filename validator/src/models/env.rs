use anyhow::Result;
use std::fmt::Debug;
use std::fmt::Display;

use im::HashMap;
use malachite::Rational;

use crate::traits::durative::Durative;
use crate::{print_assign, procedures::Procedure};

use super::method::Method;
use super::{state::State, value::Value};

/* ========================================================================== */
/*                                 Environment                                */
/* ========================================================================== */

/// Represents the current environment of the validation.
#[derive(Clone, Default)]
pub struct Env<E> {
    /// Whether debug information should be printed.
    pub verbose: bool,
    /// The end timepoint of the plan.
    pub global_end: Rational,
    /// The resolution of the temporal aspect.
    pub epsilon: Rational,
    /// Whether the time is discrete.
    pub discrete_time: bool,
    /// Whether the problem is a scheduled problem.
    pub schedule_problem: bool,
    /// The current method which is analysed.
    crt_meth: Option<Method<E>>,
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

impl<E: Debug> Debug for Env<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("verbose", &self.verbose)
            .field("global end", &self.global_end)
            .field("epsilon", &self.epsilon)
            .field("discrete_time", &self.discrete_time)
            .field("schedule_problem", &self.schedule_problem)
            .field("current method", &self.crt_meth)
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
            && self.global_end == other.global_end
            && self.epsilon == other.epsilon
            && self.discrete_time == other.discrete_time
            && self.schedule_problem == other.schedule_problem
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
    pub fn bound_fluent(&mut self, k: Vec<Value>, v: Value) -> Result<Option<Value>> {
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

    /// Checks that all bounded fluents have their value inside their bounds.
    pub fn check_bounds(&self) -> Result<()> {
        self.state.check_bounds()
    }

    /// Creates a clone of this environment extended with another.
    pub fn extends_with(&self, e: &Self) -> Self
    where
        E: Clone,
    {
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

    /// Updates the current method.
    pub fn set_method(&mut self, method: Method<E>) {
        self.crt_meth = Some(method);
    }

    /// Removes the current method.
    pub fn clear_method(&mut self) {
        self.crt_meth = None;
    }

    /// Returns the current method.
    pub fn crt_method(&self) -> Option<&Method<E>> {
        self.crt_meth.as_ref()
    }
}

impl<E: Display> Display for Env<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("\n========== Env ==========\n")?;
        f.write_fmt(format_args!("verbose = {}\n", self.verbose))?;
        f.write_fmt(format_args!("global end = {}\n", self.global_end))?;
        f.write_fmt(format_args!("epsilon = {}\n", self.epsilon))?;
        f.write_fmt(format_args!("discrete time = {}\n", self.discrete_time))?;
        f.write_fmt(format_args!("schedule problem = {}\n", self.schedule_problem))?;
        f.write_fmt(format_args!(
            "Procedures: [{}]\n",
            self.procedures
                .iter()
                .map(|(n, _)| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;
        f.write_str("\nTypes:\n")?;
        for (p, t) in self.types.iter() {
            f.write_fmt(format_args!(
                "    {}: {}\n",
                if p.is_empty() { "__obj__" } else { p },
                t.join(", ")
            ))?;
        }
        f.write_str("\nObjects:\n")?;
        for (t, o) in self.objects.iter() {
            f.write_fmt(format_args!(
                "    {}: {}\n",
                t,
                o.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(", ")
            ))?;
        }
        f.write_str("\nVariables:\n")?;
        for (n, v) in self.vars.iter() {
            f.write_fmt(format_args!("    {n} = {v}\n"))?;
        }
        f.write_fmt(format_args!("\nState:\n{}", self.state()))?;
        if let Some(method) = &self.crt_meth {
            f.write_str("\nSubtasks:\n")?;
            for (id, subtask) in method.subtasks().iter() {
                f.write_fmt(format_args!(
                    "    {}: {} {}\n",
                    id,
                    subtask.convert_to_temporal_interval(self),
                    method.name()
                ))?;
            }
        }
        f.write_str("=========================\n")
    }
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

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

    #[derive(Clone, Debug, Default)]
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
    fn bound_fluent() -> Result<()> {
        let mut s = State::default();
        s.bound(f("s"), v(true))?;
        let mut e = Env::<Mock>::default();
        e.bound_fluent(f("s"), v(true))?;
        assert_eq!(e.state, s);

        e.state = State::default();
        assert_eq!(e, Env::default());
        Ok(())
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
    fn get_fluent() -> Result<()> {
        let mut e = Env::<Mock>::default();
        e.bound_fluent(f("s"), v(true))?;
        assert_eq!(e.get_fluent(&f("s")), Some(&v(true)));
        assert_eq!(e.get_fluent(&f("a")), None);
        Ok(())
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
    fn set_state() -> Result<()> {
        let mut s = State::default();
        s.bound(f("s"), v(true))?;
        let mut e = Env::<Mock>::default();
        e.set_state(s.clone());
        assert_eq!(e.state, s);

        e.state = State::default();
        assert_eq!(e, Env::default());
        Ok(())
    }

    #[test]
    fn state() -> Result<()> {
        let mut e = Env::<Mock>::default();
        e.bound_fluent(f("s"), v(true))?;
        assert_eq!(e.state, *e.state());
        Ok(())
    }
}
