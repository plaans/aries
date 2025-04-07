//! Flatzinc types.

/// Flatzinc integer value.
///
/// ```flatzinc
/// int: p = 14;
/// ```
pub type Int = i32;

/// Convert the given boolean to [Int].
pub fn as_int(b: bool) -> Int {
    if b {
        1
    } else {
        0
    }
}
