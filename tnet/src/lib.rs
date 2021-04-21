use crate::stn::*;

pub(crate) mod distances;
pub mod stn;

/// Creates a new edge representing a maximum delay from one timepoint to another.
///  - constraint: `to - from <= max_delay`
///  - edge: from ---- (max_delay) ---> to
pub fn max_delay(from: Timepoint, to: Timepoint, max_delay: W) -> Edge {
    Edge::new(from, to, max_delay)
}

/// Creates a new edge representing a minimal delay from one timepoint to another
///  - constraint `to - from >= min_delay`
///  - edge: from <--- (-min_delay) ---- to
pub fn min_delay(from: Timepoint, to: Timepoint, min_delay: W) -> Edge {
    Edge::new(to, from, -min_delay)
}

/// Creates a new edge specifying that the first timepoint must be before the second
///  - constraint: `first <= second`
///  - edge: first <--- 0 ---- second
pub fn before_eq(first: Timepoint, second: Timepoint) -> Edge {
    min_delay(first, second, 0)
}

/// Creates a new edge specifying that the first timepoint must be strictly before the second
///  - constraint: `first < second`
///  - edge: first <--- (epsilon/step) ---- second
pub fn strictly_before(first: Timepoint, second: Timepoint) -> Edge {
    min_delay(first, second, 1)
}
