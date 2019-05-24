use crate::collection::MinVal;
use std::ops::{Index, IndexMut};

pub trait ToIndex {
    fn to_index(&self) -> usize;

    fn first_index() -> usize;
}

pub struct IndexMap<K, V> {
    pub(crate) values: Vec<V>,
    phantom: std::marker::PhantomData<K>,
}

impl<K: ToIndex, V> IndexMap<K, V> {
    pub fn empty() -> Self {
        IndexMap {
            values: Vec::new(),
            phantom: std::marker::PhantomData,
        }
    }
    pub fn new(size: usize, value: V) -> Self
    where
        V: Copy,
    {
        IndexMap {
            values: vec![value; size],
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

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn push(&mut self, v: V) -> usize {
        let id = self.values.len();
        self.values.push(v);
        id
    }

    pub fn values(&self) -> impl Iterator<Item = &V>
    where
        K: MinVal,
    {
        let start = K::min_value().to_index();
        self.values[start..].iter()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        let start = K::first_index();
        self.values[start..].iter_mut()
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
