//! Flatzinc types.

use aries::core::IntCst;

/// Flatzinc integer value.
///
/// ```flatzinc
/// int: p = 14;
/// ```
pub type Int = IntCst;

/// Convert the given boolean to [Int].
pub fn as_int(b: bool) -> Int {
    if b { 1 } else { 0 }
}
