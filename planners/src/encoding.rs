//! Functions responsible for

use aries_model::lang::IAtom;
use aries_model::literals::Lit;
use aries_planning::chronicles::{ChronicleOrigin, ChronicleTemplate, Condition, Effect, FiniteProblem, Problem, Task};

/// Iterator over all effects in an finite problem.
///
/// Each effect is associated with
/// - the ID of the chronicle instance in which the effect appears
/// - a literal that is true iff the effect is present in the solution.
pub fn effects(pb: &FiniteProblem) -> impl Iterator<Item = (usize, Lit, &Effect)> {
    pb.chronicles.iter().enumerate().flat_map(|(instance_id, ch)| {
        ch.chronicle
            .effects
            .iter()
            .map(move |eff| (instance_id, ch.chronicle.presence, eff))
    })
}

/// Iterates over all conditions in an finite problem.
///
/// Each condition is associated with a literal that is true iff the effect is present in the solution.
pub fn conditions(pb: &FiniteProblem) -> impl Iterator<Item = (Lit, &Condition)> {
    pb.chronicles.iter().flat_map(|ch| {
        ch.chronicle
            .conditions
            .iter()
            .map(move |cond| (ch.chronicle.presence, cond))
    })
}

pub const ORIGIN: i32 = 0;
pub const HORIZON: i32 = 999999;

pub struct TaskRef<'a> {
    pub presence: Lit,
    pub start: IAtom,
    pub end: IAtom,
    pub task: &'a Task,
}

/// Finds all possible refinements of a given task in the problem.
///
/// The task it the task with id `task_id` in the chronicle instance with it `chronicle_id`.
pub fn refinements_of(instance_id: usize, task_id: usize, pb: &FiniteProblem) -> Vec<TaskRef> {
    let mut supporters = Vec::new();
    let target_origin = ChronicleOrigin::Refinement { instance_id, task_id };
    for ch in pb.chronicles.iter().filter(|ch| ch.origin == target_origin) {
        let task = ch.chronicle.task.as_ref().unwrap();
        supporters.push(TaskRef {
            presence: ch.chronicle.presence,
            start: ch.chronicle.start,
            end: ch.chronicle.end,
            task,
        });
    }
    supporters
}

#[allow(clippy::ptr_arg)]
pub fn refinements_of_task<'a>(task: &Task, pb: &FiniteProblem, spec: &'a Problem) -> Vec<&'a ChronicleTemplate> {
    let mut candidates = Vec::new();
    for template in &spec.templates {
        if let Some(ch_task) = &template.chronicle.task {
            if pb.model.unifiable_seq(task.as_slice(), ch_task.as_slice()) {
                candidates.push(template);
            }
        }
    }
    candidates
}
