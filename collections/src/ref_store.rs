use itertools::Itertools;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::{Debug, Error, Formatter};
use std::hash::Hash;
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

pub trait Ref: Into<usize> + From<usize> + Copy + PartialEq {}

impl<X> Ref for X where X: Into<usize> + From<usize> + Copy + PartialEq {}

#[macro_export]
macro_rules! create_ref_type {
    ($type_name:ident) => {
        #[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Debug)]
        pub struct $type_name(std::num::NonZeroU32);

        impl $type_name {
            pub fn new(id: std::num::NonZeroU32) -> $type_name {
                $type_name(id)
            }

            pub const fn to_u32(self) -> u32 {
                self.0.get() - 1
            }
            pub const fn from_u32(u: u32) -> Self {
                unsafe { $type_name(std::num::NonZeroU32::new_unchecked(u + 1)) }
            }
        }
        impl From<usize> for $type_name {
            fn from(u: usize) -> Self {
                Self::from_u32(u as u32)
            }
        }
        impl From<$type_name> for usize {
            fn from(v: $type_name) -> Self {
                (v.0.get() - 1) as usize
            }
        }

        impl From<u64> for $type_name {
            fn from(u: u64) -> Self {
                Self::from_u32(u as u32)
            }
        }
        impl From<$type_name> for u64 {
            fn from(v: $type_name) -> Self {
                (v.0.get() - 1) as u64
            }
        }

        // ===== u32 =====
        impl From<u32> for $type_name {
            fn from(u: u32) -> Self {
                Self::from_u32(u)
            }
        }
        impl From<$type_name> for u32 {
            fn from(v: $type_name) -> Self {
                v.0.get() - 1
            }
        }

        impl<V> std::ops::Index<$type_name> for Vec<V> {
            type Output = V;

            fn index(&self, index: $type_name) -> &Self::Output {
                &self[usize::from(index)]
            }
        }

        impl<V> std::ops::IndexMut<$type_name> for Vec<V> {
            fn index_mut(&mut self, index: $type_name) -> &mut Self::Output {
                &mut self[usize::from(index)]
            }
        }
    };
}

/// A store to generate integer references to more complex values.
/// The objective is to allow interning complex values.
///
/// A new key can be obtained by `push`ing a value into the store.
///
#[derive(Clone)]
pub struct RefPool<Key, Val> {
    internal: Vec<Val>,
    rev: HashMap<Val, Key>,
}
impl<K, V: Hash + Eq> Default for RefPool<K, V> {
    fn default() -> Self {
        RefPool {
            internal: Default::default(),
            rev: HashMap::new(),
        }
    }
}
impl<K, V: Debug> Debug for RefPool<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:?}", self.internal.iter().enumerate().format(", "))
    }
}

impl<K, V> RefPool<K, V>
where
    K: Ref,
{
    pub fn len(&self) -> usize {
        self.internal.len()
    }

    pub fn is_empty(&self) -> bool {
        self.internal.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = K> {
        (0..self.len()).map(K::from)
    }

    pub fn last_key(&self) -> Option<K> {
        if self.is_empty() {
            None
        } else {
            Some((self.len() - 1).into())
        }
    }

    pub fn push(&mut self, v: V) -> K
    where
        V: Eq + Hash + Clone + Debug, // TODO: remove necessity of clone by storing reference to internal field
    {
        assert!(!self.rev.contains_key(&v), "Duplicated value: {:?}", &v);
        let id: K = self.internal.len().into();
        self.rev.insert(v.clone(), id);
        self.internal.push(v);
        id
    }

    pub fn get(&self, k: K) -> &V {
        &self.internal[k.into()]
    }

    pub fn get_ref<W: ?Sized>(&self, v: &W) -> Option<K>
    where
        W: Eq + Hash,
        V: Eq + Hash + Borrow<W>,
    {
        self.rev.get(v).copied()
    }
}

impl<K: Ref, V> Index<K> for RefPool<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index)
    }
}

/// Same as the pool but does not allow retrieving the ID of a previously interned item.
/// IDs are only returned upon insertion.
#[derive(Clone)]
pub struct RefStore<Key, Val> {
    internal: Vec<Val>,
    phantom: PhantomData<Key>,
}
impl<K, V: Debug> Debug for RefStore<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:?}", self.internal.iter().enumerate().format(", "))
    }
}

impl<K: Ref, V> Default for RefStore<K, V> {
    fn default() -> Self {
        RefStore::new()
    }
}

impl<K, V> RefStore<K, V>
where
    K: Ref,
{
    pub fn new() -> Self {
        RefStore {
            internal: Vec::new(),
            phantom: Default::default(),
        }
    }

    pub fn initialized(len: usize, v: V) -> Self
    where
        V: Clone,
    {
        RefStore {
            internal: vec![v; len],
            phantom: Default::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.internal.len()
    }

    pub fn is_empty(&self) -> bool {
        self.internal.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = K> {
        (0..self.len()).map(K::from)
    }
    pub fn entries(&self) -> impl Iterator<Item = (K, &V)> {
        self.keys().map(move |k| (k, &self[k]))
    }

    pub fn last_key(&self) -> Option<K> {
        if self.is_empty() {
            None
        } else {
            Some((self.len() - 1).into())
        }
    }

    pub fn push(&mut self, v: V) -> K {
        let id: K = self.internal.len().into();
        self.internal.push(v);
        id
    }

    pub fn get(&self, k: K) -> &V {
        &self.internal[k.into()]
    }

    pub fn get_mut(&mut self, k: K) -> &mut V {
        &mut self.internal[k.into()]
    }
}

impl<K: Ref, V> Index<K> for RefStore<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index)
    }
}

impl<K: Ref, V> IndexMut<K> for RefStore<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        self.get_mut(index)
    }
}

#[derive(Clone)]
pub struct RefVec<K, V> {
    values: Vec<V>,
    phantom: PhantomData<K>,
}

impl<K, V> Default for RefVec<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> RefVec<K, V> {
    pub fn new() -> Self {
        RefVec {
            values: Vec::new(),
            phantom: PhantomData::default(),
        }
    }

    /// Creates a new RefVec with the given `value` repeated `num_items` times.
    pub fn with_values(num_items: usize, value: V) -> Self
    where
        V: Clone,
    {
        RefVec {
            values: vec![value; num_items],
            phantom: PhantomData::default(),
        }
    }

    pub fn fill_with(&mut self, to_key: K, value_gen: impl Fn() -> V)
    where
        K: Ref,
    {
        let to_index: usize = to_key.into();
        while self.len() <= to_index {
            self.push(value_gen());
        }
    }

    pub fn contains(&self, k: K) -> bool
    where
        usize: From<K>,
    {
        usize::from(k) < self.len()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn push(&mut self, value: V) -> K
    where
        K: From<usize>,
    {
        self.values.push(value);
        K::from(self.values.len() - 1)
    }

    pub fn pop(&mut self) -> Option<V>
    where
        K: From<usize>,
    {
        self.values.pop()
    }

    /// Same as push but panics if `key` is not the result of the push.
    pub fn set_next(&mut self, key: K, value: V)
    where
        K: From<usize> + PartialEq,
    {
        let key2 = self.push(value);
        assert!(key == key2);
    }

    pub fn keys(&self) -> impl Iterator<Item = K>
    where
        K: From<usize>,
    {
        (0..(self.values.len())).map(K::from)
    }

    pub fn entries(&self) -> impl Iterator<Item = (K, &V)>
    where
        K: From<usize>,
    {
        (0..(self.values.len())).map(move |i| (K::from(i), &self.values[i]))
    }
}

impl<K: Into<usize>, V> Index<K> for RefVec<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        &self.values[index.into()]
    }
}

impl<K: Into<usize>, V> IndexMut<K> for RefVec<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        &mut self.values[index.into()]
    }
}

#[derive(Clone)]
pub struct RefMap<K, V> {
    pub(crate) entries: Vec<Option<V>>,
    phantom: PhantomData<K>,
}

impl<K, V> Default for RefMap<K, V> {
    fn default() -> Self {
        RefMap {
            entries: Vec::new(),
            phantom: Default::default(),
        }
    }
}

impl<K: Ref, V> RefMap<K, V> {
    pub fn insert(&mut self, k: K, v: V) {
        let index = k.into();
        while self.entries.len() <= index {
            self.entries.push(None);
        }
        self.entries[index] = Some(v);
    }

    /// Removes all elements from the Map.
    pub fn clear(&mut self) {
        for x in &mut self.entries {
            *x = None
        }
    }

    pub fn remove(&mut self, k: K) {
        self.entries[k.into()] = None;
    }

    pub fn contains(&self, k: K) -> bool {
        let index = k.into();
        index < self.entries.len() && self.entries[index].is_some()
    }

    pub fn get(&self, k: K) -> Option<&V> {
        let index = k.into();
        if index >= self.entries.len() {
            None
        } else {
            self.entries[index].as_ref()
        }
    }

    pub fn get_mut(&mut self, k: K) -> Option<&mut V> {
        let index = k.into();
        if index >= self.entries.len() {
            None
        } else {
            self.entries[index].as_mut()
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        (0..self.entries.len())
            .into_iter()
            .map(K::from)
            .filter(move |k| self.contains(*k))
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().filter_map(|x| x.as_ref())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.entries.iter_mut().filter_map(|x| x.as_mut())
    }

    pub fn entries(&self) -> impl Iterator<Item = (K, &V)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(idx, value)| value.as_ref().map(|v| (K::from(idx), v)))
    }

    pub fn entries_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.entries
            .iter_mut()
            .enumerate()
            .filter_map(|(idx, value)| value.as_mut().map(|v| (K::from(idx), v)))
    }
}

impl<K: Ref, V> Index<K> for RefMap<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index).expect("No such key")
    }
}

impl<K: Ref, V> IndexMut<K> for RefMap<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        self.get_mut(index).expect("No such key")
    }
}

impl<K: Ref, V> FromIterator<(K, V)> for RefMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut m = RefMap::default();
        for (k, v) in iter {
            m.insert(k, v);
        }
        m
    }
}

impl<K: Ref + Debug, V: Debug> std::fmt::Debug for RefMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (k, v) in self.entries() {
            write!(f, "{:?} -> {:?}, ", k, v)?;
        }
        write!(f, "]")
    }
}
