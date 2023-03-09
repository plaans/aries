use std::collections::HashMap;

use crate::traits::{act::Act, configurable::Configurable};

use super::{action::DurativeAction, condition::DurativeCondition, parameter::Parameter, task::Task};

/* ========================================================================== */
/*                                   Subtask                                  */
/* ========================================================================== */

#[derive(Clone, Debug)]
pub enum Subtask<E> {
    Action(DurativeAction<E>),
    Task(Task<E>),
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
