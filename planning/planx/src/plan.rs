use std::fmt::Display;
use itertools::Itertools;

use crate::{ActionRef, Object, RealValue};

type Instant = RealValue;
type Duration = RealValue;

/// A solution plan to a [`Model`](crate::Model).
pub struct SolutionPlan {
    /// Operations of the plan, without a guaranteed order.
    pub operations: Vec<Operation>,
    /// Values the plan has for the metrics defined in the model.
    pub metrics: Vec<RealValue>,
}

impl Display for SolutionPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.operations.iter().sorted_by_key(|op| op.start).format("\n")
        )?;
        if !self.metrics.is_empty() {
            write!(f, "\n; objective: {}", self.metrics.iter().format(" -- "))?;
        }
        Ok(())
    }
}

/// An operation in a plan, typically a grounding of an action.
///
/// The type of the action's arguments is generic to allow representing
/// input plans and output plans.
///
/// Can be displayed in the PDDL plan format if `OperationArg: Display`
#[derive(Debug, Clone)]
pub struct Operation {
    pub start: Instant,
    pub duration: Duration,
    pub action_ref: ActionRef,
    pub arguments: Vec<Object>,
}

impl Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:>4}: ({}{}{}) [{}]",
            self.start,
            self.action_ref,
            if self.arguments.is_empty() { "" } else { " " },
            self.arguments.iter().format(" "),
            self.duration
        )
    }
}
