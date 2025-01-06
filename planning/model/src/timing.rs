use derive_more::Display;

#[derive(Clone, Debug)]
pub struct Timestamp {
    reference: TimeRef,
    delay: i64,
}

impl Timestamp {
    pub const ORIGIN: Timestamp = Timestamp::new(TimeRef::Origin, 0);
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

#[derive(Clone, Debug, Display)]
pub enum TimeRef {
    #[display("origin")]
    Origin,
    #[display("horizon")]
    Horizon,
}

impl From<TimeRef> for Timestamp {
    fn from(value: TimeRef) -> Self {
        Timestamp::new(value, 0)
    }
}
