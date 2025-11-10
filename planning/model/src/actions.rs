use itertools::Itertools;
use std::collections::BTreeMap;

use thiserror::Error;

use crate::{env::Env, errors::ErrorMessageExt, *};

pub type ActionRef = Sym;

#[derive(Error, Debug)]
pub enum ActionsError {
    #[error("Duplicate action")]
    DuplicateAction(Sym, Sym),
    #[error("Unknown action")]
    UnkonwnAction(Sym),
}

/// Collection of actions and tasks in the problem.
///
/// An action may contain conditions, effects and subtasks, unifying the normal PDDL actions and the methods of HTN planning.
#[derive(Default)]
pub struct Actions {
    tasks: BTreeMap<TaskRef, Task>,
    actions: BTreeMap<ActionRef, Action>,
}

impl Actions {
    pub fn add(&mut self, action: Action, env: &Environment) -> Res<()> {
        if let Some(prev) = self.actions.get(&action.name) {
            return Err(action
                .name
                .invalid("Duplicate action definition")
                .info(&prev.name, "Previous definition with identical name"));
        }
        if action.name == action.achieved_task.name {
            // this a primitive action that achieves its own task
            // we should add the corresponding task
            self.add_task(action.name.clone(), action.parameters.clone()).tag(
                &action.name,
                "because this action is primitive and shoudl be the only one achieving its own task",
                None,
            )?;
        }
        self.add_achiever_to_task(&action, env)?;
        self.actions.insert(action.name.clone(), action);
        Ok(())
    }

    /// Records a new task.
    pub fn add_task(&mut self, name: TaskRef, params: Vec<Param>) -> Res<()> {
        if let Some(prev) = self.tasks.get(&name) {
            Err(name
                .invalid("This task name is already used")
                .info(&prev.name, "conflicting task definition"))
        } else {
            self.tasks.insert(
                name.clone(),
                Task {
                    name,
                    params,
                    achievers: Vec::new(),
                },
            );
            Ok(())
        }
    }

    pub fn get_task(&self, name: &TaskRef) -> Option<&Task> {
        self.tasks.get(name)
    }
    pub fn get_action(&self, name: &ActionRef) -> Option<&Action> {
        self.actions.get(name)
    }

    // Updates the achieved task to have the `action` as an achiever.
    fn add_achiever_to_task(&mut self, action: &Action, env: &Environment) -> Res<()> {
        let achieved_task = &action.achieved_task;
        let task = self
            .tasks
            .get_mut(&achieved_task.name)
            .ok_or_else(|| achieved_task.name.invalid("Unknown task"))?;
        // invariant: there should not be two achievers with the same name
        // (assert: because thise should already be checked by disallowing two identical actions)
        debug_assert!(task.achievers.iter().all(|prev| prev != &action.name));

        task.check_application(&action.achieved_task.name, &action.achieved_task.args, env)?;

        task.achievers.push(action.name.clone());
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Action> {
        self.actions.values()
    }
}

#[derive(Debug, Clone)]
pub enum Duration {
    /// Action is instantaneous (duration of 0 meaning start = end)
    Instantaneous,
    /// The action's duration should be exactly the one of the expression
    Fixed(ExprId),
    /// The action duration should within the interval (inclusive)
    Bounded(ExprId, ExprId),
    /// The action should span the same interval as its subtasks.
    /// If the action has no subtasks, it should be instantaneous.
    Subtasks,
}

pub type ActionPreferences = Preferences<Condition>;

#[derive(Debug)]
pub struct Action {
    /// Name of the action templage (e.g. "move")
    pub name: ActionRef,
    /// Typed parameters of the action (e.g. [?r: Robot, ?l: Location])
    pub parameters: Vec<Param>,
    /// Task acthieved by the action. If empty it means the task is primitive and achieves (name, parameters).
    pub achieved_task: AchievedTask,
    /// Duration requirement of the action.
    /// May notably require the action to be instantaneous to match the interval of its subtasks;
    pub duration: Duration,
    /// Timed conditions that must hold for the action to be applicable
    pub conditions: Vec<Condition>,
    /// Timed effects to occur if the action is executed.
    pub effects: Vec<Effect>,
    /// Set of named preferences in the action (typically appearing in a metric)
    pub preferences: ActionPreferences,
    /// Set of subtasks of this action (typically for actions representing HTN methods).
    pub subtasks: TaskNet,
}

impl Action {
    /// Creates a new action.
    ///
    /// Like primitive tasks in HTN, the `Action` declares that it achieves its eponymous task
    ///
    pub fn new(name: impl Into<Sym>, parameters: Vec<Param>, duration: Duration, env: &mut Environment) -> Res<Self> {
        // create the task achieved by the action.
        // Since this is a primitive action, it achieves the task corresponding to its own names and params
        let name = name.into();
        let task_args: Vec<ExprId> = parameters
            .iter()
            .map(|p| env.intern(Expr::Param(p.clone()), None))
            .try_collect()?;
        Ok(Self {
            name: name.clone(),
            parameters,
            duration,
            achieved_task: AchievedTask { name, args: task_args },
            conditions: Default::default(),
            effects: Default::default(),
            preferences: Default::default(),
            subtasks: Default::default(),
        })
    }
    pub fn instantaneous(name: impl Into<Sym>, parameters: Vec<Param>, env: &mut Environment) -> Res<Self> {
        Self::new(name, parameters, Duration::Instantaneous, env)
    }

    /// Creates a new action encoding of an HTN method. In addition to the name and parameters,
    /// it should specify the task it achieves.
    ///
    /// By default its duration is set to match the one of its subtasks (starts with the first, ends with the last).
    pub fn method(name: Sym, parameters: Vec<Param>, achieved_task: AchievedTask) -> Self {
        Action {
            name,
            parameters,
            achieved_task,
            duration: Duration::Subtasks,
            conditions: Default::default(),
            effects: Default::default(),
            preferences: Default::default(),
            subtasks: Default::default(),
        }
    }

    pub fn start(&self) -> TimeRef {
        TimeRef::ActionStart
    }

    pub fn end(&self) -> TimeRef {
        TimeRef::ActionEnd
    }

    pub fn span(&self) -> TimeInterval {
        TimeInterval::closed(self.start(), self.end())
    }
}

impl<'env> Display for Env<'env, &Action> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({})",
            self.elem.name,
            self.elem
                .parameters
                .iter()
                .map(|p| format!("{}: {}", p.name(), p.tpe()))
                .format(", ")
        )?;
        write!(f, "\n        duration: {:?}", self.elem.duration)?;

        fs(f, "conditions:", &self.elem.conditions, self.env)?;
        fs(f, "effects:", &self.elem.effects, self.env)?;
        fs(f, "preferences:", self.elem.preferences.iter(), self.env)?;
        fs(f, "vars:", self.elem.subtasks.variables.iter(), self.env)?;
        fs(f, "subtasks:", self.elem.subtasks.iter(), self.env)?;
        fs(
            f,
            "constraints:",
            self.elem.subtasks.constraints.iter().copied(),
            self.env,
        )?;
        Ok(())
    }
}
