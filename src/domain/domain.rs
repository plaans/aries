use super::range::IntRange;
use super::set::IntSet;

/// Integer domain.
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub enum IntDomain {
    IntRange(IntRange),
    IntSet(IntSet),
}

impl From<IntRange> for IntDomain {
    fn from(value: IntRange) -> Self {
        IntDomain::IntRange(value)
    }
}

impl From<IntSet> for IntDomain {
    fn from(value: IntSet) -> Self {
        IntDomain::IntSet(value)
    }
}