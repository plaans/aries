mod grounder;

use aries_solver::prelude::IntCst;
use smallvec::SmallVec;
use std::{collections::BTreeMap, sync::Arc};

use crate::{TaskId, encoder::SchedEncoder};

/// An assignment to the all parameters of a task.
///
/// Given the `n` (ordered) parameters of a task, it provides the `n` constant values that the parameters will take (typically in a particular grounding).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParametersAssignment(SmallVec<[IntCst; 4]>);

impl<T: Into<SmallVec<[IntCst; 4]>>> From<T> for ParametersAssignment {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
impl ParametersAssignment {
    pub fn get(&self) -> &[IntCst] {
        &self.0
    }
}
impl std::ops::Index<usize> for ParametersAssignment {
    type Output = IntCst;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

/// Result of the grounding process that associate each Task in [Sched] to a list of of possible assignments to its parameters.
///
/// It is guaranteed that any feasible instantiation for the parameters have feasible entry in the groundings. However some assignments appearing in the groundings
/// may not be feasible, as the grounding process relies on a relaxation of the original problem.
///
/// This structure is expected to be immutable and guarantees O(1) cloning.
pub struct Groundings {
    /// Associates each task id to its set of grounding.
    /// `None` correspond to the groundings of the empty source (which may contain global variables).
    ///
    /// TODO: At this point the structure here is no well organized and chosen only to limit disruption form the previous implementation.
    groundings: Arc<BTreeMap<Option<TaskId>, Vec<ParametersAssignment>>>,
}

impl Groundings {
    /// For each task that was the target of a grounding, provides a list of possible assignments to its parameters.
    ///
    /// If a task does not appear, it means we did not attempt to ground it, meaning it did not appear in the problem at grounding time.
    /// This may be a serious problem as it may imply that the grounding is incomplete.
    ///
    /// A task that is present with no assignments is guaranteed to be infeasible.
    /// Note that a task with no parameters should have exactly one empty assignment if feasible.
    pub fn all_task_groundings(&self) -> impl Iterator<Item = (TaskId, &[ParametersAssignment])> {
        self.groundings
            .iter()
            .filter_map(|(k, v)| k.map(|task_id| (task_id, v.as_slice())))
    }

    /// Groundings of the empty source. Since it is usually fully ground (no variables involved), there is usually exactly one, empty assignment.
    pub fn empty_source_groundings(&self) -> &[ParametersAssignment] {
        self.groundings.get(&None).map(|v| v.as_slice()).unwrap_or(&[])
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
