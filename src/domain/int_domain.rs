use crate::types::Int;

use super::range::IntRange;
use super::set::IntSet;

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

impl From<IntSet> for IntDomain {
    fn from(value: IntSet) -> Self {
        IntDomain::Set(value)
    }
}
