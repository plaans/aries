pub use cycles::*;

#[cfg(not(feature = "cycles"))]
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
