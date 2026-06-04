//! Stripped down version of the IDMap that was once in aries_solver

use std::ops::Index;
use vec_map::VecMap;

#[derive(Debug, Clone)]
pub struct IdMap<K, V> {
    internal: VecMap<V>,
    phantom: std::marker::PhantomData<K>,
}

impl<K, V> Default for IdMap<K, V> {
    fn default() -> Self {
        IdMap {
            internal: Default::default(),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<K: Into<usize>, V> IdMap<K, V> {
    pub fn insert(&mut self, k: K, v: V) {
        self.internal.insert(k.into(), v);
    }

    pub(crate) fn get(&self, k: K) -> Option<&V> {
        self.internal.get(k.into())
    }
}

impl<K: Into<usize>, V> Index<K> for IdMap<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index).expect("Key not in map")
    }
}
