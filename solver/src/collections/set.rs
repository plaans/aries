use crate::collections::ref_store::{Ref, RefMap};

use super::ref_store::IterableRefMap;

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

/// A set of values that can be converted into small unsigned integers.
/// This extends `RefSet` with a vector of all elements of the set, allowing for fast iteration
/// and clearing.
/// THe down side would be slightly slower insertion, where the set msut be queried for duplicated entries.
#[derive(Clone)]
pub struct IterableRefSet<K> {
    set: IterableRefMap<K, ()>,
}

impl<K: Ref> IterableRefSet<K> {
    pub fn new() -> IterableRefSet<K> {
        IterableRefSet {
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

impl<K: Ref> Default for IterableRefSet<K> {
    fn default() -> Self {
        Self::new()
    }
}
