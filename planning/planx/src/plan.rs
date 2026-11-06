use itertools::Itertools;
use std::fmt::Display;

use crate::{
    ActionRef, Object, RealValue,
    errors::{Span, Spanned},
};

type Instant = RealValue;
type Duration = RealValue;

/// A solution plan to a [`Model`](crate::Model).
pub struct SolutionPlan {
    /// Operations of the plan, without a guaranteed order.
    pub operations: Vec<Operation>,
    /// Values the plan has for the metrics defined in the model.
    pub metrics: Vec<RealValue>,
}

impl SolutionPlan {
    /// Creates an empty plan.
    pub fn empty() -> Self {
        SolutionPlan {
            operations: vec![],
            metrics: vec![],
        }
    }
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
#[derive(Debug, Clone)]
pub struct Operation {
    pub start: Instant,
    pub duration: Duration,
    pub action_ref: ActionRef,
    pub arguments: Vec<Object>,
    pub span: Option<Span>,
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

impl Spanned for Operation {
    fn span(&self) -> Option<&Span> {
        self.span.as_ref()
    }
}
