use crate::traits::configurable::Configurable;

use super::{action::DurativeAction, method::Method, parameter::Parameter};

/* ========================================================================== */
/*                                   Refiner                                  */
/* ========================================================================== */

#[derive(Clone, Debug)]
pub enum Refiner<E> {
    Method(Method<E>),
    Action(DurativeAction<E>),
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
}

impl<E: Clone> Configurable<E> for Task<E> {
    fn params(&self) -> &[Parameter] {
        self.params.as_ref()
    }
}
