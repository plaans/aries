use crate::collections::ref_store::{Ref, RefMap};

/// A set of values that can be converted into small unsigned integers.
#[derive(Clone)]
pub struct RefSet<K> {
    set: RefMap<K, ()>,
}

impl<K: Ref> RefSet<K> {
    pub fn new() -> RefSet<K> {
        RefSet {
            set: Default::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    pub fn insert(&mut self, k: K) {
        self.set.insert(k, ());
    }

    pub fn remove(&mut self, k: K) {
        self.set.remove(k);
    }

    pub fn clear(&mut self) {
        self.set.clear()
    }

    pub fn contains(&self, k: K) -> bool {
        self.set.contains(k)
    }

    pub fn iter(&self) -> impl Iterator<Item = K> + '_
    where
        K: From<usize>,
    {
        self.set.entries().map(|(k, _)| k)
    }
}

impl<K: Ref> Default for RefSet<K> {
    fn default() -> Self {
        Self::new()
    }
}
