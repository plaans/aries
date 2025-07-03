use derive_more::Display;

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct Timestamp {
    reference: TimeRef,
    delay: i64,
}

impl Timestamp {
    /// Temporal origin of the problem (nothing changes before)
    pub const ORIGIN: Timestamp = Timestamp::new(TimeRef::Origin, 0);
    /// Temporal horizon of the problem (nothing changes after)
    pub const HORIZON: Timestamp = Timestamp::new(TimeRef::Horizon, 0);

    pub const fn new(reference: TimeRef, delay: i64) -> Self {
        Self { reference, delay }
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.delay.cmp(&0) {
            std::cmp::Ordering::Less => write!(f, "{} - {}", self.reference, -self.delay),
            std::cmp::Ordering::Equal => write!(f, "{}", self.reference),
            std::cmp::Ordering::Greater => write!(f, "{} + {}", self.reference, self.delay),
        }
    }
}

#[derive(Clone, Debug, Display, PartialEq, PartialOrd, Ord, Eq)]
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
        Timestamp::new(value, 0)
    }
}

/// Represents a temporal interval, composed of a start and end timestamps
#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct TimeInterval {
    pub start: Timestamp,
    pub end: Timestamp,
}

impl TimeInterval {
    pub fn at(tp: impl Into<Timestamp>) -> Self {
        let tp = tp.into();
        TimeInterval::closed(tp.clone(), tp)
    }
    pub fn closed(start: impl Into<Timestamp>, end: impl Into<Timestamp>) -> Self {
        TimeInterval {
            start: start.into(),
            end: end.into(),
        }
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
