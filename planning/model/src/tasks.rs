use std::collections::BTreeMap;

use itertools::Itertools;

use crate::{errors::*, *};

pub type TaskRef = Sym;

/// Describes a declared task as the ones in standard HTN models.
#[derive(Debug)]
pub struct Task {
    pub name: TaskRef,
    pub params: Vec<Param>,
    /// All actions achieving this task
    pub achievers: Vec<ActionRef>,
}

impl Task {
    pub fn num_params(&self) -> usize {
        self.params.len()
    }

    // Check that a subtask `name(args...)` has correctly typed arguments (same number and types).
    // The `name` should be the same as the name of the and is only used to generate locali
    pub fn check_application(&self, name: &Sym, args: &[ExprId], env: &Environment) -> Res<()> {
        assert_eq!(&self.name, name, "Cannot check application on this task");
        let num_params = self.num_params().max(args.len());
        // For this we consider the max length of the arguments and parameters
        // For each we will check that there is no missing param/arg and if not that they have compatible types.
        for i in 0..num_params {
            // check that enough arguments are provided
            if i > args.len() {
                return Err(name
                    .invalid("Not enough arguments in task")
                    .info(&self.params[i].name, "missing parameter"));
            }
            let arg = args[i];
            // check that no extra arguments are given
            if i > self.num_params() {
                return Err((env / arg)
                    .invalid("unexpected argument")
                    .info(&self.name, "for this task definition"));
            }
            let param = &self.params[i];

            // check that the argument has a compatible type
            param.tpe.accepts(arg, env).msg(env)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct AchievedTask {
    pub name: TaskRef,
    pub args: Vec<ExprId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubtaskId(usize);

impl SubtaskId {
    pub fn start(self) -> TimeRef {
        TimeRef::TaskStart(self)
    }
    pub fn end(self) -> TimeRef {
        TimeRef::TaskEnd(self)
    }
}

impl Display for SubtaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "_st_{}", self.0)
    }
}

#[derive(Debug)]
pub struct Subtask {
    /// Original name of the task (human readable)
    pub ref_name: Option<Sym>,
    /// Name of the task it represents
    pub task_name: TaskRef,
    pub args: Vec<ExprId>,
    pub source: Option<Span>,
}

impl Display for Env<'_, &Subtask> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref_name) = &self.elem.ref_name {
            write!(f, "{ref_name}: ")?;
        }
        write!(
            f,
            "{}({})",
            self.elem.task_name,
            self.elem.args.iter().map(|a| self.env / *a).join(", ")
        )
    }
}

impl Spanned for Env<'_, &Subtask> {
    fn span(&self) -> Option<&Span> {
        self.elem.source.as_ref()
    }
}

#[derive(Default, Debug)]
pub struct TaskSet {
    next_id: usize,
    tasks: BTreeMap<SubtaskId, Subtask>,
    name_to_id: BTreeMap<Sym, SubtaskId>,
}

impl TaskSet {
    pub fn add(&mut self, task: Subtask, env: &Environment) -> Res<SubtaskId> {
        let id = SubtaskId(self.next_id);
        if let Some(ref_new) = &task.ref_name {
            if let Some(prev_task) = self.get_by_ref(ref_new.canonical_str()) {
                return Err(ref_new
                    .invalid("task ID is already used")
                    .info(env / prev_task, "previous usage"));
            }
            self.name_to_id.insert(ref_new.clone(), id);
        }
        // increase next_id, only done now that we have considered all possible failures
        self.next_id += 1;

        self.tasks.insert(id, task);

        Ok(id)
    }

    pub fn get_by_ref(&self, name: &str) -> Option<&Subtask> {
        self.name_to_id.get(name).map(|tid| self.tasks.get(tid).unwrap())
    }

    pub fn get(&self, id: SubtaskId) -> Option<&Subtask> {
        self.tasks.get(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (SubtaskId, &Subtask)> + '_ {
        self.tasks.iter().map(|(k, v)| (*k, v))
    }
}
