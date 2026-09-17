mod grounder;

use std::{collections::BTreeMap, sync::Arc};

use crate::{TaskId, analysis::SourceGrounding, encoder::SchedEncoder};

/// Result of the grounding process that associate each Task in [Sched] to a list of of possible assignments to its parameters.
///
/// It is guaranteed that any feasible instantiation for the parameters have feasible entry in the groundings. However some assignments appearing in the groundings
/// may not be feasible, as the grounding process relies on a relaxation of the original problem.
pub struct Groundings {
    /// Associates each task id to its set of grounding.
    /// `None` correspond to the groundings of the empty source (which may contain global variables).
    ///
    /// TODO: At this point the structure here is no well organized and chosen only to limit disruption form the previous implementation.
    groundings: Arc<BTreeMap<Option<TaskId>, Vec<SourceGrounding>>>,
}

impl Groundings {
    /// For each task that was the target of a grounding, provides a list of possible assignments to its parameters.
    ///
    /// If a task does not appear, it means we did not attempt to ground it, meaning it did not appear in the problem at grounding time.
    /// This may be a serious problem as it may imply that the grounding is incomplete.
    ///
    /// A task that is present with no assignments is guaranteed to be infeasible.
    /// Note that a task with no parameters should have exactly one empty assignment if feasible.
    pub fn all_task_groundings(&self) -> impl Iterator<Item = (TaskId, &[SourceGrounding])> {
        self.groundings
            .iter()
            .filter_map(|(k, v)| k.map(|task_id| (task_id, v.as_slice())))
    }
}

/// Ground all tasks appear in this problem.
///
/// IMPORTANT: it must be the case that all causal links have been popuplated in the [`SchedEncoder`].
/// Indeed, the grounding process relies on the handling of the `HasValueAt` constraints to infer conditions of tasks.
pub fn ground_all_tasks(sched: &SchedEncoder) -> Groundings {
    let grounder = grounder::Grounder::from(sched);
    // grounder.print_datalog_program();
    let groundings = grounder.run();

    let groundings: BTreeMap<_, _> = groundings.into_iter().collect();

    Groundings {
        groundings: Arc::new(groundings),
    }
}
