use std::ops::Index;

use idmap::intid::IntegerId;

use crate::*;

#[derive(Debug, Clone, Copy, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct TaskId(u32);

impl idmap::intid::IntegerId for TaskId {
    idmap::intid::impl_newtype_id_body!(for TaskId(u32));
}

/// Task
#[derive(Clone)]
pub struct Task {
    /// An optional identifier for the task that allows referring to it unambiguously.
    pub name: Sym,
    /// Time reference at which the task must start
    pub start: Time,
    /// Time reference at which the task must end
    pub end: Time,
    /// Arguments of the task
    pub args: Vec<Atom>,
    /// Presence of the task, true iff it appears in the solution
    pub presence: Lit,
}
impl Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?},{:?}] {:?}{:?}", self.start, self.end, self.name, self.args)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct Tasks {
    tasks: DirectIdMap<TaskId, Task>,
    next_id: u32,
}

impl Tasks {
    pub fn insert(&mut self, task: Task) -> TaskId {
        let id = TaskId::from_int(self.next_id);
        self.tasks.insert(id, task);
        self.next_id += 1;
        id
    }

    pub fn iter(&self) -> impl Iterator<Item = &Task> {
        self.tasks.iter().map(|(_k, v)| v)
    }
}

impl Index<TaskId> for Tasks {
    type Output = Task;

    fn index(&self, index: TaskId) -> &Self::Output {
        &self.tasks[index]
    }
}
