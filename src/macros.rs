/// Given three types A, B and C with the following traits:
/// - From<B> for A, From<C> for B,
/// - TryFrom<A> for B, TryFrom<B> for C
///
/// The macro implements the traits:
///  - From<C> for A
///  - TryFrom<A> for C
#[macro_export]
macro_rules! transitive_conversions {
    ($A: ty, $B: ty, $C: ty) => {
        impl From<$C> for $A {
            fn from(i: $C) -> Self {
                <$B>::from(i).into()
            }
        }

        impl TryFrom<$A> for $C {
            type Error = anyhow::Error;

            fn try_from(value: $A) -> Result<Self, Self::Error> {
                match <$B>::try_from(value) {
                    Ok(x) => <$C>::try_from(x),
                    Err(x) => Err(x),
                }
            }
        }
    };
}

/// Given three types A, B and C with the following traits:
/// - From<B> for A, From<C> for B,
///
/// The macro implements the traits:
///  - From<C> for A
#[macro_export]
macro_rules! transitive_conversion {
    ($A: ty, $B: ty, $C: ty) => {
        impl From<$C> for $A {
            fn from(i: $C) -> Self {
                <$B>::from(i).into()
            }
        }
    };
}