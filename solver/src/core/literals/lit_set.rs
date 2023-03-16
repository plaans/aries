use crate::core::*;
use std::array::TryFromSliceError;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

/// Set of literals.
///
/// The set is said to contain a literal `l` iff
///  - `l` was previously inserted in the set; or
///  - another literal `l2` was previously inserted such that `l2.entails(l)`.
///
/// # Limitations
///
/// The implementation is optimized for small, dynamic sets as it stores literals in an unsorted vector.
/// A specialized implementation for large sets could be implemented with a `Map<VarBound, BoundValue>`
///
/// # Example
/// ```
/// use aries::core::literals::LitSet;
/// use aries::core::VarRef;
/// let mut set = LitSet::new();
/// let var = VarRef::from_u32(3); // arbitrary variable
/// assert!(!set.contains(var.leq(0)));
/// set.insert(var.leq(0));
/// assert!(set.contains(var.leq(0)));
/// assert!(set.contains(var.leq(1))); // present because entailed by `var.leq(0)`
/// assert!(!set.contains(var.leq(-1))); // not present as it is not entailed
/// ```
#[derive(Clone, Debug, Default)]
pub struct LitSet {
    elements: HashMap<SignedVar, UpperBound>,
}

impl LitSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        LitSet {
            elements: HashMap::with_capacity(capacity),
        }
    }

    pub fn into_sorted(self) -> StableLitSet {
        StableLitSet::new(self)
    }

    pub fn literals(&self) -> impl Iterator<Item = Lit> + '_ {
        self.elements.iter().map(|(var, val)| Lit::from_parts(*var, *val))
    }

    pub fn contains(&self, elem: Lit) -> bool {
        self.elements
            .get(&elem.svar())
            .map_or(false, |b| b.stronger(elem.bound_value()))
    }

    /// Insert a literal `lit` into the set.
    ///
    /// Note that all literals directly implied by `lit` are also implicitly inserted.
    pub fn insert(&mut self, lit: Lit) {
        #[allow(clippy::or_fun_call)]
        let val = self.elements.entry(lit.svar()).or_insert(lit.bound_value());
        if lit.bound_value().strictly_stronger(*val) {
            *val = lit.bound_value()
        }
    }

    /// Remove a literal `rm` from the set.
    ///
    /// Removal needs to account for implicit literals.
    ///
    /// `{ (X <= 1) } is equivalent to { (X <= 1), (X <= 2), (X <= 3), ...}`
    /// Hence, if we remove (X <= 2) from this set, we are left with
    /// `{ (X <= 3), (X <= 4), ... }`, which is normalized to `{ (X <= 3) }`
    ///
    /// When doing this operation, it is likely that the resulting literal would be tautological.
    /// For instance, consider is a boolean variable `B` whose value is either `0` or `1`.
    /// Removing `(B <= 0)` from the set `{ (B <= 0) }` would result in the set `{ (B <= 1) }`.
    /// The literal `(B <= 1)` is tautological and can be ignored in the set.
    /// The method will use the method `tautological` to determine which literals are always true.
    ///
    pub fn remove(&mut self, rm: Lit, tautology: impl Fn(Lit) -> bool) {
        debug_assert!(self.contains(rm));
        let weaker = Lit::from_parts(rm.svar(), rm.bound_value() + BoundValueAdd::RELAXATION);
        if tautology(weaker) {
            self.elements.remove(&rm.svar());
        } else {
            self.elements.insert(rm.svar(), weaker.bound_value());
        }
    }
}

impl<T: IntoIterator<Item = Lit>> From<T> for LitSet {
    fn from(lits: T) -> Self {
        let mut set = LitSet::new();
        for l in lits {
            set.insert(l);
        }
        set
    }
}

/// A set of literal in a canonical form and which can thus be used for comparison.
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct StableLitSet {
    elements: Vec<Lit>,
}

impl StableLitSet {
    pub const EMPTY: Self = Self { elements: vec![] };

    pub fn new(set: LitSet) -> Self {
        let mut elements: Vec<Lit> = set.literals().collect();
        elements.sort();
        Self { elements }
    }

    pub fn literals(&self) -> impl Iterator<Item = Lit> + '_ {
        self.elements.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

impl<T: IntoIterator<Item = Lit>> From<T> for StableLitSet {
    fn from(lits: T) -> Self {
        Self::new(lits.into())
    }
}

impl<const N: usize> TryFrom<&StableLitSet> for [Lit; N] {
    type Error = TryFromSliceError;

    fn try_from(value: &StableLitSet) -> Result<Self, Self::Error> {
        value.elements.as_slice().try_into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const A: VarRef = VarRef::from_u32(1);
    const B: VarRef = VarRef::from_u32(2);
    const C: VarRef = VarRef::from_u32(3);

    #[test]
    fn test_lit_set() {
        let mut set = LitSet::new();

        assert!(!set.contains(A.leq(1)));
        assert!(!set.contains(A.geq(1)));
        assert_eq!(set.elements.len(), 0);

        set.insert(A.leq(1));
        assert_eq!(set.elements.len(), 1);
        assert!(set.contains(A.leq(1)));
        assert!(set.contains(A.leq(2)));
        assert!(!set.contains(A.leq(0)));

        // insert (A <= 2) that should (implicitly) already be in the set
        set.insert(A.leq(2));
        assert_eq!(set.elements.len(), 1);
        assert!(set.contains(A.leq(1)));
        assert!(set.contains(A.leq(2)));
        assert!(!set.contains(A.leq(0)));
        assert!(!set.contains(B.leq(10)));
        assert!(!set.contains(C.leq(10)));

        // insert (A <= 0) that should subsume the (A <= 1) literal present in the set
        set.insert(A.leq(0));
        assert_eq!(set.elements.len(), 1);
        assert!(set.contains(A.leq(1)));
        assert!(set.contains(A.leq(2)));
        assert!(set.contains(A.leq(0)));
        assert!(!set.contains(A.leq(-1)));
        assert!(!set.contains(B.leq(10)));
        assert!(!set.contains(C.leq(10)));

        set.insert(B.geq(5));
        assert_eq!(set.elements.len(), 2);
        set.insert(C.geq(5));
        assert_eq!(set.elements.len(), 3);
        set.insert(C.geq(3));
        assert!(set.contains(A.leq(0)));
        assert!(!set.contains(A.leq(-1)));
        assert!(set.contains(B.geq(5)));
        assert!(!set.contains(B.geq(6)));
        assert!(set.contains(C.geq(5)));
        assert!(!set.contains(C.geq(6)));
        assert!(!set.contains(B.leq(10)));
        assert!(!set.contains(C.leq(10)));
    }

    #[test]
    fn test_lit_set_removal() {
        let mut set = LitSet::new();

        let tauto = |l| A.leq(4).entails(l);

        assert!(!set.contains(A.leq(1)));
        assert!(!set.contains(A.geq(1)));
        assert_eq!(set.elements.len(), 0);

        set.insert(A.leq(1));
        assert_eq!(set.elements.len(), 1);
        assert!(set.contains(A.leq(1)));
        assert!(set.contains(A.leq(2)));
        assert!(!set.contains(A.leq(0)));

        set.remove(A.leq(1), tauto);
        assert_eq!(set.elements.len(), 1);
        assert!(!set.contains(A.leq(1)));
        assert!(set.contains(A.leq(2)));
        assert!(!set.contains(A.leq(0)));

        set.remove(A.leq(3), tauto);
        assert_eq!(set.elements.len(), 0);
    }
}
