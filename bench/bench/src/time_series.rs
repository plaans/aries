use std::time::Duration;

use itertools::Itertools;

pub type Timestamp = Duration;

pub struct TimeSerie {
    /// A non-empty and *unsorted* list of increments
    /// The first one define both the start of the line and its initial value.
    /// All subsequent one define an increment at a later point in time
    /// The first data point gives the
    increments: Vec<Increment>,
    /// Time after which the value is undefined
    end: Timestamp,
}

impl TimeSerie {
    pub fn constant(value: f64, start: Timestamp, end: Timestamp) -> Self {
        Self {
            increments: vec![Increment::new(start, value)],
            end,
        }
    }

    pub fn line(&self) -> (Vec<f64>, Vec<f64>) {
        let mut incs = self.increments.clone();
        incs.sort_unstable_by_key(|inc| inc.time);
        incs.dedup_by(|a, b| {
            if a.time == b.time {
                b.delta += a.delta;
                true
            } else {
                false
            }
        });
        let mut xs = Vec::with_capacity(incs.len());
        let mut ys = Vec::with_capacity(incs.len());
        let mut curr = 0.0;
        for (i, inc) in incs.iter().enumerate() {
            curr += inc.delta;
            xs.push(inc.time.as_secs_f64());
            ys.push(curr);
            // add a point to make the sure we have steps (and not linear interpolations) and that the line persists until the end
            let next = incs.get(i + 1).map(|next| next.time).unwrap_or(self.end);
            xs.push(next.as_secs_f64());
            ys.push(curr);
        }
        (xs, ys)
    }

    pub fn from_constant_per_part(parts: impl IntoIterator<Item = (Timestamp, f64)>, end: Timestamp) -> Self {
        // collect all parts and make sure they are sorted (important for transforming into increments)
        let mut parts = parts.into_iter().collect_vec();
        parts.sort_by_key(|(t, _)| *t);

        let mut prev_val = 0.0;
        let mut increments = Vec::with_capacity(parts.len());
        for (t, v) in parts {
            if t > end {
                break; // clip values beyond the end
            }
            let inc = v - prev_val;
            prev_val = v;
            increments.push(Increment::new(t, inc));
        }
        Self { increments, end }
    }

    pub fn start(&self) -> Timestamp {
        self.increments.first().unwrap().time
    }

    pub fn end(&self) -> Timestamp {
        self.end
    }

    /// Interval (inclusive) over which the time serie is defined
    pub fn bounds(&self) -> (Timestamp, Timestamp) {
        (self.start(), self.end())
    }

    pub fn with_prefix(mut self, start: Duration, value: f64) -> Self {
        if self.start() > start {
            self.increments.insert(0, Increment::new(start, value));
            // at the previous start, adjust the delta
            self.increments[1].delta -= value;
        }
        self
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut Increment> {
        self.increments.iter_mut()
    }
}

impl std::ops::AddAssign<TimeSerie> for TimeSerie {
    fn add_assign(&mut self, rhs: TimeSerie) {
        assert_eq!(
            self.bounds(),
            rhs.bounds(),
            "Cannot add two time series defined on different domains."
        );
        self.increments.extend(rhs.increments);
    }
}
impl std::ops::DivAssign<f64> for TimeSerie {
    fn div_assign(&mut self, rhs: f64) {
        assert!(rhs.is_finite());
        self.increments.iter_mut().for_each(|i| i.delta /= rhs);
    }
}
impl std::ops::Add<TimeSerie> for TimeSerie {
    type Output = TimeSerie;

    fn add(mut self, rhs: TimeSerie) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::ops::Neg for TimeSerie {
    type Output = TimeSerie;

    fn neg(mut self) -> Self::Output {
        self.iter_mut().for_each(|i| i.delta = -i.delta);
        self
    }
}

impl std::ops::Add<TimeSerie> for f64 {
    type Output = TimeSerie;

    fn add(self, mut rhs: TimeSerie) -> Self::Output {
        rhs.iter_mut().for_each(|i| i.delta += self);
        rhs
    }
}
impl std::ops::Sub<TimeSerie> for f64 {
    type Output = TimeSerie;

    fn sub(self, mut rhs: TimeSerie) -> Self::Output {
        rhs.iter_mut().for_each(|i| i.delta = self - i.delta);
        rhs
    }
}

#[derive(Copy, Clone)]
struct Increment {
    time: Timestamp,
    delta: f64,
}

impl Increment {
    pub fn new(time: Timestamp, delta: f64) -> Self {
        Self { time, delta }
    }
}
