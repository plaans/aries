use std::fmt::Display;

use crate::{ConditionAnnotations, ConditionId, Env, ExprId, TimeInterval, Timestamp};

#[derive(Clone, Debug)]
pub struct Condition {
    /// Identifier of the condition, unique within its action (preserved by cloning).
    pub id: ConditionId,
    pub interval: TimeInterval,
    pub cond: ExprId,
    /// Additional information on the condition, typically set by preprocessing steps.
    pub annotations: ConditionAnnotations,
}

impl Condition {
    /// Creates a new condition over an interval
    pub fn over(itv: impl Into<TimeInterval>, cond: impl Into<ExprId>) -> Self {
        Condition {
            id: ConditionId::fresh(),
            interval: itv.into(),
            cond: cond.into(),
            annotations: Default::default(),
        }
    }

    /// Creates a new condition at the indicated timepoint
    pub fn at(tp: impl Into<Timestamp>, cond: impl Into<ExprId>) -> Self {
        Self::over(TimeInterval::at(tp), cond)
    }
}

impl<'a> Display for Env<'a, &Condition> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.elem.interval, (self.env / self.elem.cond))
    }
}
