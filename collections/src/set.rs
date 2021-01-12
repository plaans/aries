use std::marker::PhantomData;

/// A set of values that can be converted into small unsigned integers.
/// The underlying implementation uses a bitset to keep track of the values present in the set.
pub struct RefSet<K> {
    set: bit_set::BitSet,
    _phantom: PhantomData<K>,
}

impl<K: Into<usize>> RefSet<K> {
    pub fn new() -> RefSet<K> {
        RefSet {
            set: Default::default(),
            _phantom: Default::default(),
        }
    }

    pub fn insert(&mut self, k: K) {
        self.set.insert(k.into());
    }

    pub fn remove(&mut self, k: K) {
        self.set.remove(k.into());
    }

    pub fn clear(&mut self) {
        self.set.clear()
    }

    pub fn contains(&self, k: K) -> bool {
        self.set.contains(k.into())
    }
}

impl<K: Into<usize>> Default for RefSet<K> {
    fn default() -> Self {
        Self::new()
    }
}
