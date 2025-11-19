//! Various datastructures specialized for the handling of literals (watchlists, sets, clauses, implication graph, ...)

use std::{borrow::Borrow, ops::Deref};

pub use disjunction::*;
pub use implication_graph::*;
pub use lit_set::*;
use smallvec::SmallVec;
pub use watches::*;

use crate::core::Lit;

mod disjunction;
mod implication_graph;
mod lit_set;
mod watches;

const INLINE_SIZE: usize = 3;

/// A sequence of literals, optimized to represent at least 3 elements inline.
///
/// In the base case (when [`IntCst`] is 32 bits), it should only occupy 4 machine words (one more that a `Vec<Lit>`).
#[derive(PartialEq, PartialOrd, Ord, Eq, Clone, Debug, Hash)]
pub struct Lits {
    elems: SmallVec<[Lit; INLINE_SIZE]>,
}

impl Lits {
    pub fn with_capacity(n: usize) -> Self {
        Self {
            elems: SmallVec::with_capacity(n),
        }
    }

    pub fn from_slice(lits: impl Borrow<[Lit]>) -> Self {
        Self {
            elems: SmallVec::from_slice(lits.borrow()),
        }
    }

    /// Creates a new `Lits` from a vector, reusing the allocation from the vector
    pub fn from_vec(lits: Vec<Lit>) -> Lits {
        let elems = if lits.len() <= INLINE_SIZE {
            // can be represented inline, the Vec and its allocation will be dropped.
            // we do this because SmallVec would always reuse the allocation may thus fail to place the elements inline.
            SmallVec::from_slice(&lits)
        } else {
            // use specialized method that will reuse the allocation from the vector
            SmallVec::from_vec(lits)
        };
        Self { elems }
    }

    pub fn into_vec(self) -> Vec<Lit> {
        self.elems.into_vec()
    }

    pub fn into_boxed_slice(self) -> Box<[Lit]> {
        self.elems.into_boxed_slice()
    }

    pub fn push(&mut self, item: Lit) {
        self.elems.push(item);
    }

    pub fn extend_from_slice(&mut self, items: &[Lit]) {
        self.elems.extend_from_slice(items);
    }

    pub fn clear(&mut self) {
        self.elems.clear();
    }

    /// Keep only those literals that match the predicate.
    pub fn retain<F: FnMut(Lit) -> bool>(&mut self, mut f: F) {
        self.elems.retain(move |l| f(*l));
    }

    pub fn iter(&self) -> impl Iterator<Item = Lit> + '_ {
        self.elems.iter().copied()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Lit> + '_ {
        self.elems.iter_mut()
    }

    /// Simplify a *disjunction*, removing redundant literals if the literals are to be interpreted in a disjunctive form.
    /// At the end of this operations, the literals will be sorted.
    pub(crate) fn simplify_disjunctive(&mut self) {
        // sort literals, so that they are grouped by (1) variable and (2) affected bound
        // We can use an unstable sort (potentially faster) as to equal elements are undistinguishable.
        self.elems.sort_unstable();
        // remove duplicated literals
        let mut i = 0;
        while i + 1 < self.elems.len() {
            let elem = self.elems[i];
            let next = self.elems[i + 1];
            // because of the ordering properties, we can only check entailment for the immediately following element
            if elem.entails(next) || elem == Lit::FALSE {
                self.elems.remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn as_slice(&self) -> &[Lit] {
        &self.elems
    }

    fn new() -> Self {
        Self { elems: SmallVec::new() }
    }
}

impl AsRef<[Lit]> for Lits {
    fn as_ref(&self) -> &[Lit] {
        &self.elems
    }
}
impl Deref for Lits {
    type Target = [Lit];

    fn deref(&self) -> &Self::Target {
        &self.elems
    }
}
impl AsMut<[Lit]> for Lits {
    fn as_mut(&mut self) -> &mut [Lit] {
        &mut self.elems
    }
}
impl FromIterator<Lit> for Lits {
    fn from_iter<T: IntoIterator<Item = Lit>>(iter: T) -> Self {
        Self {
            elems: SmallVec::from_iter(iter),
        }
    }
}

impl<'a> IntoIterator for &'a Lits {
    type Item = Lit;

    type IntoIter = std::iter::Copied<std::slice::Iter<'a, Lit>>;

    fn into_iter(self) -> Self::IntoIter {
        self.elems.as_slice().iter().copied()
    }
}
impl IntoIterator for Lits {
    type Item = Lit;
    type IntoIter = smallvec::IntoIter<[Lit; INLINE_SIZE]>;

    fn into_iter(self) -> Self::IntoIter {
        self.elems.into_iter()
    }
}

#[cfg(test)]
mod test {
    use crate::core::{literals::Lits, Lit};

    #[test]
    fn test_lits_size() {
        if std::mem::size_of::<Lit>() == 8 {
            assert_eq!(std::mem::size_of::<Lits>(), 32);
        }
    }
}
