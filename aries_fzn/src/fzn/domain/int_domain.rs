use crate::fzn::domain::IntSet;
use crate::fzn::types::Int;

use super::range::IntRange;

/// Intger domain.
///
/// ```flatzinc
/// var 1..5: x;
/// ```
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub enum IntDomain {
    Singleton(Int),
    Range(IntRange),
    Set(IntSet),
}

impl From<IntRange> for IntDomain {
    fn from(value: IntRange) -> Self {
        IntDomain::Range(value)
    }
}
