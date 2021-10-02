use crate::literals::Lit;

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
/// use aries_model::literals::LitSet;
/// use aries_model::lang::VarRef;
/// let mut set = LitSet::empty();
/// let var = VarRef::from_u32(3); // arbitrary variable
/// assert!(!set.contains(var.leq(0)));
/// set.insert(var.leq(0));
/// assert!(set.contains(var.leq(0)));
/// assert!(set.contains(var.leq(1))); // present because entailed by `var.leq(0)`
/// assert!(!set.contains(var.leq(-1))); // not present as it is not entailed
/// ```
#[derive(Default)]
pub struct LitSet {
    elements: Vec<Lit>,
}

impl LitSet {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        LitSet {
            elements: Vec::with_capacity(capacity),
        }
    }

    pub fn contains(&self, elem: Lit) -> bool {
        self.elements.iter().any(|l| l.entails(elem))
    }

    pub fn insert(&mut self, elem: Lit) {
        for l in self.elements.iter_mut() {
            if l.entails(elem) {
                return;
            } else if elem.entails(*l) {
                *l = elem;
                return;
            }
        }
        self.elements.push(elem)
    }
}

#[cfg(test)]
mod test {
    use crate::lang::VarRef;
    use crate::literals::LitSet;

    const A: VarRef = VarRef::from_u32(0);
    const B: VarRef = VarRef::from_u32(1);
    const C: VarRef = VarRef::from_u32(2);

    #[test]
    fn test_lit_set() {
        let mut set = LitSet::empty();

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
}
