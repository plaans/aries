use super::range::IntRange;
use super::set::IntSet;

// TODO: trait for domain?

/// Integer domain.
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub enum IntDomain {
    IntRange(IntRange),
    IntSet(IntSet),
}