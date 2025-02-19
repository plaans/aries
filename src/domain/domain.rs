use super::range::IntRange;
use super::set::IntSet;

// TODO: trait for domain?

/// Integer domain.
pub enum IntDomain {
    IntRange(IntRange),
    IntSet(IntSet),
}