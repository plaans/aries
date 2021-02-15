use std::ops::Add;

pub struct Instant(std::time::Instant);

pub const SUPPORT_CPU_TIMING: bool = true;

impl Instant {
    pub fn now() -> Self {
        Instant(std::time::Instant::now())
    }

    pub fn elapsed(&self) -> Duration {
        Duration(self.0.elapsed())
    }
}

pub struct Duration(std::time::Duration);

impl Duration {
    pub fn zero() -> Self {
        Duration(std::time::Duration::from_nanos(0))
    }

    pub fn as_secs(&self) -> Option<f64> {
        Some(self.0.as_secs_f64())
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.6}", self.0.as_secs_f64())
    }
}

impl std::ops::Add for Duration {
    type Output = Duration;

    fn add(self, rhs: Self) -> Self::Output {
        Duration(self.0.add(rhs.0))
    }
}

impl std::ops::AddAssign for Duration {
    fn add_assign(&mut self, rhs: Self) {
        *self = Duration(self.0.add(rhs.0))
    }
}
