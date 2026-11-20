use derive_more::Display;
use std::{
    fmt::Display,
    ops::{Add, Sub},
};

use crate::{RealValue, SubtaskId};

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct Timestamp {
    pub reference: TimeRef,
    pub delay: RealValue,
    /// Number of time steps to be added to the delay.
    pub num_dt: i32,
}

/// Symbolical representation of a time step.
pub struct Dt;

impl Timestamp {
    /// Temporal origin of the problem (nothing changes before)
    pub const ORIGIN: Timestamp = Timestamp::new(TimeRef::Origin, RealValue::ZERO);
    /// Temporal horizon of the problem (nothing changes after)
    pub const HORIZON: Timestamp = Timestamp::new(TimeRef::Horizon, RealValue::ZERO);

    pub const fn new(reference: TimeRef, delay: RealValue) -> Self {
        Self {
            reference,
            delay,
            num_dt: 0,
        }
    }
}

impl Add<Dt> for Timestamp {
    type Output = Timestamp;

    fn add(mut self, _: Dt) -> Self::Output {
        self.num_dt += 1;
        self
    }
}
impl Sub<Dt> for Timestamp {
    type Output = Timestamp;

    fn sub(mut self, _: Dt) -> Self::Output {
        self.num_dt -= 1;
        self
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.delay.cmp(&RealValue::ZERO) {
            std::cmp::Ordering::Less => write!(f, "{} - {}", self.reference, -self.delay),
            std::cmp::Ordering::Equal => write!(f, "{}", self.reference),
            std::cmp::Ordering::Greater => write!(f, "{} + {}", self.reference, self.delay),
        }?;
        match self.num_dt {
            0 => Ok(()),
            1 => write!(f, "+dt"),
            -1 => write!(f, "-dt"),
            pos if pos > 0 => write!(f, "+{pos}dt"),
            neg => write!(f, "-{}dt", -neg),
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
pub struct TimeInterval {
    pub start: Timestamp,
    pub end: Timestamp,
}

impl TimeInterval {
    pub const FULL: TimeInterval = TimeInterval {
        start: Timestamp::ORIGIN,
        end: Timestamp::HORIZON,
    };

    pub fn at(tp: impl Into<Timestamp>) -> Self {
        let tp = tp.into();
        TimeInterval::closed(tp, tp)
    }
    pub fn open(start: impl Into<Timestamp>, end: impl Into<Timestamp>) -> Self {
        Self {
            start: start.into() + Dt,
            end: end.into() - Dt,
        }
    }
    pub fn closed(start: impl Into<Timestamp>, end: impl Into<Timestamp>) -> Self {
        TimeInterval {
            start: start.into(),
            end: end.into(),
        }
    }
    pub fn as_timestamp(&self) -> Option<Timestamp> {
        if self.start == self.end { Some(self.start) } else { None }
    }
}

impl Display for TimeInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(at) = self.as_timestamp() {
            return write!(f, "[{at}]");
        }
        if self.start.num_dt == 1 {
            write!(f, "({}, ", self.start - Dt)?;
        } else {
            write!(f, "[{}, ", self.start)?;
        }
        if self.end.num_dt == -1 {
            write!(f, "{})", self.end + Dt)
        } else {
            write!(f, "{}]", self.end)
        }
    }
}
