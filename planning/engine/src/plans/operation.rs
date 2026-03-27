use planx::{ActionRef, errors::Span};
use timelines::IntCst;

/// Absolute time, associated to an operation start/end
pub type Instant = IntCst;

/// Duration (difference between two instants)
pub type Duration = IntCst;

/// An operation in a plan, typically a grounding of an action.
#[derive(Debug, Clone)]
pub struct Operation<OperationArg> {
    pub start: Instant,
    pub duration: Duration,
    pub action_ref: ActionRef,
    pub arguments: Vec<OperationArg>,
    #[allow(unused)]
    pub span: Option<Span>,
}
