use derive_more::Display;
use std::fmt::Display;

use crate::{RealValue, SubtaskId};

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct Timestamp {
    pub reference: TimeRef,
    pub delay: RealValue,
}

impl Timestamp {
    /// Temporal origin of the problem (nothing changes before)
    pub const ORIGIN: Timestamp = Timestamp::new(TimeRef::Origin, RealValue::ZERO);
    /// Temporal horizon of the problem (nothing changes after)
    pub const HORIZON: Timestamp = Timestamp::new(TimeRef::Horizon, RealValue::ZERO);

    pub const fn new(reference: TimeRef, delay: RealValue) -> Self {
        Self { reference, delay }
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.delay.cmp(&RealValue::ZERO) {
            std::cmp::Ordering::Less => write!(f, "{} - {}", self.reference, -self.delay),
            std::cmp::Ordering::Equal => write!(f, "{}", self.reference),
            std::cmp::Ordering::Greater => write!(f, "{} + {}", self.reference, self.delay),
        }
    }
}

#[derive(Copy, Clone, Debug, Display, PartialEq, PartialOrd, Ord, Eq)]
pub enum TimeRef {
    #[display("origin")]
    Origin,
    #[display("horizon")]
    Horizon,
    #[display("start")]
    ActionStart,
    #[display("end")]
    ActionEnd,
    #[display("start({_0})")]
    TaskStart(SubtaskId),
    #[display("end({_0})")]
    TaskEnd(SubtaskId),
}

impl From<TimeRef> for Timestamp {
    fn from(value: TimeRef) -> Self {
        Timestamp::new(value, RealValue::ZERO)
    }
}

impl From<RealValue> for Timestamp {
    fn from(value: RealValue) -> Self {
        Timestamp::new(TimeRef::Origin, value)
    }
}

/// Represents a temporal interval, composed of a start and end timestamps
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub enum TimeInterval {
    Closed { start: Timestamp, end: Timestamp },
    Open { start: Timestamp, end: Timestamp },
}

impl TimeInterval {
    pub const FULL: TimeInterval = TimeInterval::Closed {
        start: Timestamp::ORIGIN,
        end: Timestamp::HORIZON,
    };

    pub fn at(tp: impl Into<Timestamp>) -> Self {
        let tp = tp.into();
        TimeInterval::closed(tp, tp)
    }
    pub fn open(start: impl Into<Timestamp>, end: impl Into<Timestamp>) -> Self {
        TimeInterval::Open {
            start: start.into(),
            end: end.into(),
        }
    }
    pub fn closed(start: impl Into<Timestamp>, end: impl Into<Timestamp>) -> Self {
        TimeInterval::Closed {
            start: start.into(),
            end: end.into(),
        }
    }
    pub fn as_timestamp(&self) -> Option<Timestamp> {
        match self {
            TimeInterval::Closed { start, end } if start == end => Some(*start),
            _ => None,
        }
    }
}

impl Display for TimeInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeInterval::Closed { start, end } if start == end => {
                write!(f, "[{}]", start)
            }
            TimeInterval::Closed { start, end } => {
                write!(f, "[{}, {}]", start, end)
            }
            TimeInterval::Open { start, end } => {
                write!(f, "({}, {})", start, end)
            }
        }
    }
}
