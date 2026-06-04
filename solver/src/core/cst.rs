pub use types::*;

// Default int types, enabled if all int features are disabled.
#[cfg(all(not(feature = "i64"), not(feature = "i128")))]
mod types {
    /// Type representing an integer constant.
    pub type IntCst = i32;

    /// Type used to store the result of operations on `IntCst` that may overflow
    pub type LongCst = i64;

    /// Name of the `IntCst` underlying type
    pub const INT_TYPE_NAME: &str = "i32";
}

#[cfg(all(feature = "i64", not(feature = "i128")))]
mod types {
    /// Type representing an integer constant.
    pub type IntCst = i64;

    /// Type used to store the result of operations on `IntCst` that may overflow
    pub type LongCst = i128;

    /// Name of the `IntCst` underlying type
    pub const INT_TYPE_NAME: &str = "i64";
}

#[cfg(feature = "i128")]
mod types {
    /// Type representing an integer constant.
    pub type IntCst = i128;

    /// Type used to store the result of operations on `IntCst` that may overflow
    pub type LongCst = i128;

    /// Name of the `IntCst` underlying type
    pub const INT_TYPE_NAME: &str = "i128";
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
pub(crate) fn cst_long_to_int_clamped(cst: LongCst) -> IntCst {
    cst.clamp(INT_CST_MIN as LongCst, INT_CST_MAX as LongCst) as IntCst
}

/// Overflow tolerant max value for integer constants.
/// It is used as a default for the upper bound of integer variable domains.
///
/// A larger value can be select by enlarging the [`IntCst`] representation  with the `i64` and `i128` cargo features
pub const INT_CST_MAX: IntCst = IntCst::MAX / 4 - 1;

/// Overflow tolerant min value for integer constants.
/// It is used as a default for the lower bound of integer variable domains
///
/// A larger value can be select by enlarging the [`IntCst`] representation  with the `i64` and `i128` cargo features
pub const INT_CST_MIN: IntCst = -INT_CST_MAX;

/// Represents a type that can typically be converted into an [`IntCst`] and is accepted in arithmetic operations.
///
/// THe types that we typically want for this is `IntCst` itself and `usize` because
/// many CP programs use indices for domain bounds.
///
/// Thus we want to support operations on `usize` as dealing explictly with the conversion
/// `usize -> IntCst` substantially obfuscate the expressed logic.
/// In virtually all case where usize is an index, it can be safelly converted into an `IntCst` max whose minimal value
/// is in the order of `2^29`.
///
/// We cannot simply implement of `IntCst` and `usize` because when that is the case, and `IntCst` is 64 or 128 bit,
/// the compiler does not know wheter to interpret a numeric literal as `IntCst` or `usize`. We thus also need it to be implemented
/// for `i32` the default for int literals.
pub(crate) trait IntoIntCst {
    /// Converts the value into an [`IntCst`],
    /// panicking if the value is not in `[INT_CST_MIN, INT_CST_MAX]`.
    fn into_int_cst(self) -> IntCst;
}

macro_rules! into_int_cst {
    ($type_name:ident) => {
        impl IntoIntCst for $type_name {
            fn into_int_cst(self) -> IntCst {
                let val: IntCst = self.try_into().expect("not representable");
                assert!(val <= INT_CST_MAX);
                assert!(val >= INT_CST_MIN);
                val
            }
        }
    };
}

into_int_cst!(i32);
into_int_cst!(i64);
into_int_cst!(i128);
into_int_cst!(usize);
into_int_cst!(u32);
into_int_cst!(u64);
