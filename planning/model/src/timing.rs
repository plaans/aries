use derive_more::Display;

use crate::RealValue;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct Timestamp {
    reference: TimeRef,
    delay: RealValue,
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
    Start,
    #[display("end")]
    End,
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
    pub const FULL: TimeInterval = TimeInterval::new(Timestamp::ORIGIN, Timestamp::HORIZON);

    pub const fn new(start: Timestamp, end: Timestamp) -> Self {
        Self { start, end }
    }

    pub fn at(tp: impl Into<Timestamp>) -> Self {
        let tp = tp.into();
        TimeInterval::closed(tp, tp)
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
        if self.start == self.end {
            write!(f, "[{}]", self.start)
        } else {
            write!(f, "[{}, {}]", self.start, self.end)
        }
    }
}
