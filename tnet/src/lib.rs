use crate::num::*;
use crate::stn::*;

pub mod num;
pub mod stn;

/// Creates a new edge representing a maximum delay from one timepoint to another.
///  - constraint: `to - from <= max_delay`
///  - edge: from ---- (max_delay) ---> to
pub fn max_delay<W: Time>(from: NodeID, to: NodeID, max_delay: W) -> Edge<W> {
    Edge::new(from, to, max_delay)
}

/// Creates a new edge representing a minimal delay from one timepoint to another
///  - constraint `to - from >= min_delay`
///  - edge: from <--- (-min_delay) ---- to
pub fn min_delay<W: Time>(from: NodeID, to: NodeID, min_delay: W) -> Edge<W> {
    Edge::new(to, from, -min_delay)
}

/// Creates a new edge specifying that the first timepoint must be before the second
///  - constraint: `first <= second`
///  - edge: first <--- 0 ---- second
pub fn before_eq<W: Time>(first: NodeID, second: NodeID) -> Edge<W> {
    min_delay(first, second, W::zero())
}

/// Creates a new edge specifying that the first timepoint must be strictly before the second
///  - constraint: `first < second`
///  - edge: first <--- (epsilon/step) ---- second
pub fn strictly_before<W: Time>(first: NodeID, second: NodeID) -> Edge<W> {
    min_delay(first, second, W::step())
}
