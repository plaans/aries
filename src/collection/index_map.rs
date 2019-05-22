use std::ops::{Index, IndexMut};

pub trait ToIndex {
    fn to_index(&self) -> usize;
}

pub struct IndexMap<K, V> {
    pub(crate) values: Vec<V>,
    phantom: std::marker::PhantomData<K>,
}

impl<K: ToIndex, V> IndexMap<K, V> {
    pub fn new(size: usize, value: V) -> Self
    where
        V: Copy,
    {
        let mut vec = Vec::new();
        vec.resize(size, value);
        IndexMap {
            values: vec,
            phantom: std::marker::PhantomData,
        }
    }
    pub fn new_with<F: FnMut() -> V>(size: usize, generator: F) -> Self {
        let mut vec = Vec::new();
        vec.resize_with(size, generator);
        IndexMap {
            values: vec,
            phantom: std::marker::PhantomData,
        }
    }

    fn get(&self, k: K) -> &V {
        &self.values[k.to_index()]
    }
    fn get_mut(&mut self, k: K) -> &mut V {
        &mut self.values[k.to_index()]
    }
    pub fn write(&mut self, k: K, v: V) {
        self.values[k.to_index()] = v;
    }
}

impl<K: ToIndex, V> Index<K> for IndexMap<K, V> {
    type Output = V;
    fn index(&self, k: K) -> &Self::Output {
        &self.values[k.to_index()]
    }
}
impl<K: ToIndex, V> IndexMut<K> for IndexMap<K, V> {
    fn index_mut(&mut self, k: K) -> &mut Self::Output {
        &mut self.values[k.to_index()]
    }
}
