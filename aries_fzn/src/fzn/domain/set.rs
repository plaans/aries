use std::hash::Hash;
use std::ops::Deref;

use crate::fzn::types::Int;

/// Generic set defined by its values.
///
/// ```
/// use aries_fzn::fzn::domain::Set;
///
/// let set = Set::from_iter([0,4,4,8,3,0,3]);
///
/// assert_eq!(set.lb(), &0);
/// assert_eq!(set.ub(), &8);
/// assert_eq!(set.len(), 4);
///
/// let values: Vec<i32> = set.iter().copied().collect();
///
/// assert_eq!(values, vec![0,3,4,8]);
/// ```
#[derive(Hash, Clone, Eq, PartialEq, Debug)]
pub struct Set<T>
where
    T: Hash + Ord,
{
    // Vec has less overhead than HashSet.
    // It should be better for small sets.
    values: Vec<T>,
}

impl<T> Set<T>
where
    T: Hash + Ord,
{
    /// Return a new empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return `true` if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Return the number of elements in the set.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Return the lower bound of the set.
    ///
    /// Panic if empty.
    pub fn lb(&self) -> &T {
        &self.values[0]
    }

    /// Return the upper bound of the set.
    ///
    /// Panic if empty.
    pub fn ub(&self) -> &T {
        &self.values[self.len() - 1]
    }

    /// Return both bounds of the set.
    pub fn bounds(&self) -> (&T, &T) {
        (self.lb(), self.ub())
    }

    /// Return values of the set.
    pub fn values(&self) -> &Vec<T> {
        &self.values
    }
}

impl<T> Deref for Set<T>
where
    T: Hash + Ord,
{
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.values
    }
}

impl<T> FromIterator<T> for Set<T>
where
    T: Hash + Ord,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut values: Vec<T> = iter.into_iter().collect();
        values.sort_unstable();
        values.dedup();
        Self { values }
    }
}

impl<T> Default for Set<T>
where
    T: Hash + Ord,
{
    fn default() -> Self {
        Self {
            values: Default::default(),
        }
    }
}

/// Set of integers.
///
/// ```flatzinc
/// var {0,3,4,8}: x;
/// ```
pub type IntSet = Set<Int>;
