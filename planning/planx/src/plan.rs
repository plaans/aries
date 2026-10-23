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
