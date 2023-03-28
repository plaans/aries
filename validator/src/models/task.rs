use crate::traits::{configurable::Configurable, durative::Durative};

use super::{action::DurativeAction, method::Method, parameter::Parameter};

/* ========================================================================== */
/*                                   Refiner                                  */
/* ========================================================================== */

#[derive(Clone, Debug)]
pub enum Refiner<E> {
    Method(Method<E>),
    Action(DurativeAction<E>),
}

impl<E> Refiner<E> {
    pub fn id(&self) -> &String {
        match self {
            Refiner::Method(m) => m.id(),
            Refiner::Action(a) => a.id(),
        }
    }
}

impl<E> Durative<E> for Refiner<E> {
    fn start(&self, env: &super::env::Env<E>) -> &super::time::Timepoint {
        match self {
            Refiner::Method(m) => m.start(env),
            Refiner::Action(a) => a.start(env),
        }
    }

    fn end(&self, env: &super::env::Env<E>) -> &super::time::Timepoint {
        match self {
            Refiner::Method(m) => m.end(env),
            Refiner::Action(a) => a.end(env),
        }
    }

    fn is_start_open(&self) -> bool {
        match self {
            Refiner::Method(m) => m.is_start_open(),
            Refiner::Action(a) => a.is_start_open(),
        }
    }

    fn is_end_open(&self) -> bool {
        match self {
            Refiner::Method(m) => m.is_end_open(),
            Refiner::Action(a) => a.is_end_open(),
        }
    }
}

/* ========================================================================== */
/*                                    Task                                    */
/* ========================================================================== */

/// Represents an abstract task in the hierarchy.
#[derive(Clone, Debug)]
pub struct Task<E> {
    /// The name of the task.
    name: String,
    /// The identifier of the task that might be used to refer to it (e.g. in HTN plans).
    id: String,
    /// The parameters of the task.
    params: Vec<Parameter>,
    /// The method or the action that refine the task.
    refiner: Refiner<E>,
}

impl<E> Task<E> {
    pub fn new(name: String, id: String, params: Vec<Parameter>, refiner: Refiner<E>) -> Self {
        Self {
            name,
            id,
            params,
            refiner,
        }
    }

    pub fn id(&self) -> &String {
        &self.id
    }

    pub fn refiner(&self) -> &Refiner<E> {
        &self.refiner
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

impl<E: Clone> Configurable<E> for Task<E> {
    fn params(&self) -> &[Parameter] {
        self.params.as_ref()
    }
}
