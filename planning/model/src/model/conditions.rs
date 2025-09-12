use std::fmt::Display;

use crate::{Env, ExprId, TimeInterval, Timestamp};

#[derive(Clone, Debug)]
pub struct Condition {
    pub interval: TimeInterval,
    pub cond: ExprId,
}

impl Condition {
    /// Creates a new condition over an interval
    pub fn over(itv: impl Into<TimeInterval>, cond: impl Into<ExprId>) -> Self {
        Condition {
            interval: itv.into(),
            cond: cond.into(),
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
