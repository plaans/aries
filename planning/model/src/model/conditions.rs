use derive_more::derive::Display;

use crate::{TimeInterval, Timestamp, TypedExpr};

#[derive(Clone, Debug, Display)]
#[display("{} {}", interval, cond)]
pub struct Condition {
    pub interval: TimeInterval,
    pub cond: TypedExpr,
}

impl Condition {
    /// Creates a new condition over an interval
    pub fn over(itv: impl Into<TimeInterval>, cond: impl Into<TypedExpr>) -> Self {
        Condition {
            interval: itv.into(),
            cond: cond.into(),
        }
    }

    /// Creates a new condition at the indicated timepoint
    pub fn at(tp: impl Into<Timestamp>, cond: impl Into<TypedExpr>) -> Self {
        Self::over(TimeInterval::at(tp), cond)
    }
}
