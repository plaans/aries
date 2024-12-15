pub use types::*;

#[cfg(all(feature = "i64", feature = "i128"))]
compile_error!("features i64 and i128 are mutually exclusive.");

// Default int types, enabled if all int features are disabled.
//
// This module is also enabled for any invalid feature configurations,
// in order to suppress cascading compile-time errors.
// Instead of relying on these errors we have a custom error for invalid
// configurations (see above).
#[cfg(any(
    all(feature = "i64", feature = "i128"),
    all(not(feature = "i64"), not(feature = "i128"))
))]
mod types {
    /// Type representing an integer constant.
    pub type IntCst = i32;

    /// Type used to store the result of operations on `IntCst` that may overflow
    pub type LongCst = i64;
}

#[cfg(all(feature = "i64", not(feature = "i128")))]
mod types {
    /// Type representing an integer constant.
    pub type IntCst = i64;

    /// Type used to store the result of operations on `IntCst` that may overflow
    pub type LongCst = i128;
}

#[cfg(all(feature = "i128", not(feature = "i64")))]
mod types {
    /// Type representing an integer constant.
    pub type IntCst = i128;

    /// Type used to store the result of operations on `IntCst` that may overflow
    pub type LongCst = i128;
}

/// Convert IntCst to LongCst
pub(crate) const fn cst_int_to_long(cst: IntCst) -> LongCst {
    cst as LongCst
}

/// Convert u32 to IntCst
pub const fn u32_to_cst(cst: u32) -> IntCst {
    cst as IntCst
}

/// Convert LongCst to IntCst
pub(crate) const fn cst_long_to_int(cst: LongCst) -> IntCst {
    cst as IntCst
}

/// Overflow tolerant max value for integer constants.
/// It is used as a default for the upper bound of integer variable domains
pub const INT_CST_MAX: IntCst = IntCst::MAX / 4 - 1;

/// Overflow tolerant min value for integer constants.
/// It is used as a default for the lower bound of integer variable domains
pub const INT_CST_MIN: IntCst = -INT_CST_MAX;
