use std::fmt::Display;

use itertools::Itertools;
use planx::{
    ActionRef,
    errors::{Span, Spanned},
};
use timelines::IntCst;

/// Absolute time, associated to an operation start/end
pub type Instant = IntCst;

/// Duration (difference between two instants)
pub type Duration = IntCst;

/// An operation in a plan, typically a grounding of an action.
///
/// The type of the action's arguments is generic to allow representing
/// input plans and output plans.
///
/// Can be displayed in the PDDL plan format if `OperationArg: Display`
#[derive(Debug, Clone)]
pub struct Operation<OperationArg> {
    pub start: Instant,
    pub duration: Duration,
    pub action_ref: ActionRef,
    pub arguments: Vec<OperationArg>,
    #[allow(unused)]
    pub span: Option<Span>,
}

impl<Arg: Display> Display for Operation<Arg> {
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

impl<Arg: Display> Spanned for Operation<Arg> {
    fn span(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}
