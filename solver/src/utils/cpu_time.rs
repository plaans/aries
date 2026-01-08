//! This module provides a way to count elapsed CPU cycles  on x86_64 platforms.
//! The implementation relies on time-stamp counters which are very fast but can be quite brittle.
//! This is only active if the feature `cpu_cycles` is on and the target arch is `x86_64`
//! Otherwise, we provide a dummy implementation that does nothing.

pub use cycles::*;

#[cfg(any(not(feature = "cpu_cycles"), not(target_arch = "x86_64")))]
mod cycles {
    use std::fmt::{Display, Formatter, Result};

    pub struct StartCycleCount();
    pub const SUPPORT_CPU_TIMING: bool = false;

    impl StartCycleCount {
        pub fn now() -> Self {
            StartCycleCount()
        }

        pub fn elapsed(&self) -> CycleCount {
            CycleCount()
        }
    }

    #[derive(Copy, Clone)]
    pub struct CycleCount();

    impl CycleCount {
        pub fn zero() -> Self {
            CycleCount()
        }

        pub fn count(&self) -> Option<u64> {
            None
        }
    }

    impl Default for CycleCount {
        fn default() -> Self {
            Self::zero()
        }
    }

    impl std::fmt::Display for CycleCount {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "???")
        }
    }

    impl std::ops::Add for CycleCount {
        type Output = CycleCount;

        fn add(self, _: Self) -> Self::Output {
            CycleCount()
        }
    }

    impl std::ops::AddAssign for CycleCount {
        fn add_assign(&mut self, _: Self) {}
    }

    impl std::ops::Div for CycleCount {
        type Output = CycleRatio;

        fn div(self, _: Self) -> Self::Output {
            CycleRatio()
        }
    }

    pub struct CycleRatio();

    impl Display for CycleRatio {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "")
        }
    }
}

#[cfg(all(feature = "cpu_cycles", target_arch = "x86_64"))]
mod cycles {
    use std::fmt::{Display, Formatter, Result};

    use core::arch::x86_64 as arch;

    /// Returns the value of the processor's time-stamp counter.
    /// This does not wait for any other instruction to finish. (see rdtscp for this).
    unsafe fn now() -> u64 {
        unsafe { arch::_rdtsc() }
    }

    #[derive(Copy, Clone)]
    pub struct StartCycleCount(u64);
    pub const SUPPORT_CPU_TIMING: bool = true;

    impl StartCycleCount {
        pub fn now() -> Self {
            unsafe { StartCycleCount(now()) }
        }

        pub fn elapsed(&self) -> CycleCount {
            unsafe { CycleCount(now() - self.0) }
        }
    }

    #[derive(Copy, Clone)]
    pub struct CycleCount(u64);

    impl CycleCount {
        pub fn zero() -> Self {
            CycleCount(0)
        }

        pub fn count(&self) -> Option<u64> {
            Some(self.0)
        }
    }

    impl Default for CycleCount {
        fn default() -> Self {
            Self::zero()
        }
    }

    impl std::fmt::Display for CycleCount {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::ops::Add for CycleCount {
        type Output = CycleCount;

        fn add(self, other: Self) -> Self::Output {
            CycleCount(self.0 + other.0)
        }
    }

    impl std::ops::AddAssign for CycleCount {
        fn add_assign(&mut self, other: Self) {
            *self = *self + other
        }
    }

    impl std::ops::Div for CycleCount {
        type Output = CycleRatio;

        fn div(self, other: Self) -> Self::Output {
            CycleRatio((self.0 as f64) / (other.0 as f64))
        }
    }

    pub struct CycleRatio(f64);

    impl Display for CycleRatio {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "{:.2} %", self.0 * 100f64)
        }
    }
}
