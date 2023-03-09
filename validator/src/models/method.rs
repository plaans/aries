use std::collections::HashMap;

use crate::traits::{configurable::Configurable, durative::Durative};

use super::{
    action::DurativeAction,
    condition::DurativeCondition,
    env::Env,
    parameter::Parameter,
    task::Task,
    time::{Timepoint, TimepointKind},
};

/* ========================================================================== */
/*                                   Subtask                                  */
/* ========================================================================== */

#[derive(Clone, Debug)]
pub enum Subtask<E> {
    Action(DurativeAction<E>),
    Task(Task<E>),
}

impl<E> Durative<E> for Subtask<E> {
    fn start(&self, env: &Env<E>) -> &Timepoint {
        match self {
            Subtask::Action(a) => a.start(env),
            Subtask::Task(t) => t.refiner().start(env),
        }
    }

    fn end(&self, env: &Env<E>) -> &Timepoint {
        match self {
            Subtask::Action(a) => a.end(env),
            Subtask::Task(t) => t.refiner().end(env),
        }
    }

    fn is_start_open(&self) -> bool {
        match self {
            Subtask::Action(a) => a.is_start_open(),
            Subtask::Task(t) => t.refiner().is_start_open(),
        }
    }

    fn is_end_open(&self) -> bool {
        match self {
            Subtask::Action(a) => a.is_end_open(),
            Subtask::Task(t) => t.refiner().is_end_open(),
        }
    }
}

/* ========================================================================== */
/*                                   Method                                   */
/* ========================================================================== */

/// Represents a method to decompose a task.
#[derive(Clone, Debug)]
pub struct Method<E> {
    /// The name of the method.
    name: String,
    /// The identifier of the method that might be used to refer to it (e.g. in HTN plans).
    id: String,
    /// The parameters of the method.
    params: Vec<Parameter>,
    /// The conditions and the constraints for the method to be applicable.
    conditions: Vec<DurativeCondition<E>>,
    /// The list of subtasks to decompose the method.
    subtasks: HashMap<String, Subtask<E>>,
    /// The default start timepoint to return if there is no subtasks.
    default_start: Timepoint,
    /// The default end timepoint to return if there is no subtasks.
    default_end: Timepoint,
}

impl<E> Method<E> {
    pub fn new(
        name: String,
        id: String,
        params: Vec<Parameter>,
        conditions: Vec<DurativeCondition<E>>,
        subtasks: HashMap<String, Subtask<E>>,
    ) -> Self {
        Self {
            name,
            id,
            params,
            conditions,
            subtasks,
            default_start: Timepoint::fixed((-1).into()),
            default_end: Timepoint::new(TimepointKind::GlobalEnd, 1.into()),
        }
    }

    pub fn id(&self) -> &String {
        &self.id
    }

    pub fn subtasks(&self) -> &HashMap<String, Subtask<E>> {
        &self.subtasks
    }
}

impl<E: Clone> Configurable<E> for Method<E> {
    fn params(&self) -> &[Parameter] {
        self.params.as_ref()
    }
}

impl<E> Durative<E> for Method<E> {
    fn start(&self, env: &Env<E>) -> &Timepoint {
        let mut min = env.global_end.clone();
        let mut timepoint = None;
        for (_, subtask) in self.subtasks.iter() {
            let start = subtask.start(env);
            let evaluation = start.eval(Some(self), env);
            if evaluation < min && evaluation >= 0 {
                min = evaluation;
                timepoint = Some(start);
            }
        }
        timepoint.unwrap_or(&self.default_start)
    }

    fn end(&self, env: &Env<E>) -> &Timepoint {
        let mut max = 0.into();
        let mut timepoint = None;
        for (_, subtask) in self.subtasks.iter() {
            let end = subtask.end(env);
            let evaluation = end.eval(Some(self), env);
            if evaluation > max && evaluation <= env.global_end {
                max = evaluation;
                timepoint = Some(end);
            }
        }
        timepoint.unwrap_or(&self.default_end)
    }

    fn is_start_open(&self) -> bool {
        false
    }

    fn is_end_open(&self) -> bool {
        false
    }
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {

    use crate::models::{env::Env, task::Refiner, time::Timepoint};

    use super::*;

    #[derive(Debug, Default)]
    struct MockExpr {}

    fn a(n: &str, s: Timepoint, e: Timepoint) -> DurativeAction<MockExpr> {
        DurativeAction::new(n.into(), n.into(), vec![], vec![], vec![], s, e)
    }
    fn st_a(n: &str, s: Timepoint, e: Timepoint) -> Subtask<MockExpr> {
        Subtask::Action(a(n, s, e))
    }
    fn m(n: &str, st: HashMap<String, Subtask<MockExpr>>) -> Method<MockExpr> {
        Method::new(n.into(), n.into(), vec![], vec![], st)
    }
    fn st_m(n: &str, st: HashMap<String, Subtask<MockExpr>>) -> Subtask<MockExpr> {
        let tn = n.replace("m", "t");
        Subtask::Task(Task::new(tn.clone(), tn, vec![], Refiner::Method(m(n, st))))
    }
    fn t(i: i32) -> Timepoint {
        Timepoint::fixed(i.into())
    }

    #[test]
    fn timepoints_empty_simple() {
        let env = Env::default();
        let mth = m("m", HashMap::new());
        assert_eq!(*mth.start(&env), t(-1));
        assert_eq!(
            *mth.end(&env),
            Timepoint::new(crate::models::time::TimepointKind::GlobalEnd, 1.into())
        );
    }

    #[test]
    fn timepoints_empty_complex() {
        let env = Env::default();
        let m1 = st_m("m1", HashMap::new());
        let m2 = st_m("m2", HashMap::new());
        let mth = m("m", HashMap::from([("s1".into(), m1), ("s2".into(), m2)]));
        assert_eq!(*mth.start(&env), t(-1));
        assert_eq!(
            *mth.end(&env),
            Timepoint::new(crate::models::time::TimepointKind::GlobalEnd, 1.into())
        );
    }

    #[test]
    fn timepoints_simple() {
        let mut env = Env::default();
        env.global_end = 302.into();
        let s1 = t(0);
        let e1 = t(100);
        let a1 = st_a("a1", s1.clone(), e1);
        let s2 = t(101);
        let e2 = t(251);
        let a2 = st_a("a2", s2, e2);
        let s3 = t(252);
        let e3 = t(302);
        let a3 = st_a("a3", s3, e3.clone());
        let mth = m(
            "m",
            HashMap::from([("s1".into(), a1), ("s2".into(), a2), ("s3".into(), a3)]),
        );
        assert_eq!(*mth.start(&env), s1);
        assert_eq!(*mth.end(&env), e3);
    }

    #[test]
    fn timepoints_complex() {
        let mut env = Env::default();
        env.global_end = 100.into();
        let a11 = st_a("a11", t(1), t(10));
        let m12 = st_m("m12", HashMap::new());
        let m1 = st_m("m1", HashMap::from([("s1".into(), a11), ("s2".into(), m12)]));
        let a2 = st_a("a2", t(11), t(20));
        let a31 = st_a("a31", t(21), t(30));
        let a321 = st_a("a321", t(31), t(40));
        let a322 = st_a("a322", t(41), t(50));
        let m32 = st_m("m32", HashMap::from([("s1".into(), a321), ("s2".into(), a322)]));
        let a331 = st_a("a331", t(51), t(60));
        let a332 = st_a("a332", t(61), t(70));
        let m33 = st_m("m33", HashMap::from([("s1".into(), a331), ("s2".into(), a332)]));
        let m3 = st_m(
            "m3",
            HashMap::from([("s1".into(), a31), ("s2".into(), m32), ("s3".into(), m33)]),
        );
        let mth = m(
            "m",
            HashMap::from([("s1".into(), m1), ("s2".into(), a2), ("s3".into(), m3)]),
        );
        assert_eq!(*mth.start(&env), t(1));
        assert_eq!(*mth.end(&env), t(70));
    }
}
