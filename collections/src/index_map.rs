use crate::MinVal;
use std::ops::{Index, IndexMut};

pub trait ToIndex {
    fn to_index(&self) -> usize;

    fn first_index() -> usize;
}
impl ToIndex for u32 {
    fn to_index(&self) -> usize {
        (*self) as usize
    }
    fn first_index() -> usize {
        0
    }
}

pub struct IndexMap<K, V> {
    pub values: Vec<V>, // TODO: make private
    phantom: std::marker::PhantomData<K>,
}

impl<K: ToIndex, V> IndexMap<K, V> {
    pub fn new(size: usize, value: V) -> Self
    where
        V: Copy,
    {
        IndexMap {
            values: vec![value; size + K::first_index()],
            phantom: std::marker::PhantomData,
        }
    }
    pub fn new_with<F: FnMut() -> V>(size: usize, generator: F) -> Self {
        let mut vec = Vec::with_capacity(size + K::first_index());
        vec.resize_with(size + K::first_index(), generator);
        IndexMap {
            values: vec,
            phantom: std::marker::PhantomData,
        }
    }

    pub fn num_elems(&self) -> usize where {
        self.values.len() - K::first_index()
    }

    pub fn raw_len(&self) -> usize where {
        self.values.len()
    }

    pub fn scan<F: Fn(&V) -> bool>(&self, from_index: usize, matches: F) -> Option<usize> {
        let mut i = from_index;
        while i < self.values.len() {
            if matches(&self.values[i]) {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    pub fn overwrite(&mut self, idx: usize, v: V) {
        self.values[idx] = v
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

    pub fn get(&self, k: K) -> &V {
        &self.values[k.to_index()]
    }
    pub fn get_mut(&mut self, k: K) -> &mut V {
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
