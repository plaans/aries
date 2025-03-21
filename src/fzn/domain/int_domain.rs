use crate::fzn::types::Int;

use super::range::IntRange;

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub enum IntDomain {
    Singleton(Int),
    Range(IntRange),
}

impl From<IntRange> for IntDomain {
    fn from(value: IntRange) -> Self {
        IntDomain::Range(value)
    }
}
