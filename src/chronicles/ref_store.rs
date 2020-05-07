// TODO : change into bidirectionnal map


use std::marker::PhantomData;
use std::collections::HashMap;
use std::ops::Index;
use std::hash::Hash;
use std::borrow::Borrow;
use std::fmt::{Debug, Formatter, Error};
use itertools::Itertools;


pub trait Ref: Into<usize> + From<usize> + Copy {}

impl<X> Ref for X
    where X: Into<usize> + From<usize> + Copy
{}

/// A store to generate integer references to more complex values.
/// The objective is to allow interning complex values.
///
/// A new key can be obtained by `push`ing a value into the store.
///
#[derive(Clone)]
pub struct RefStore<Key,Val>  {
    internal: Vec<Val>,
    rev: HashMap<Val,Key>,
}
impl<K,V> Default for RefStore<K,V>
 where V: Eq + Hash {
    fn default() -> Self {
        RefStore { internal: Default::default(), rev: Default::default() }
    }
}
impl<K,V: Debug> Debug for RefStore<K,V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", format!("{:?}", self.internal.iter().enumerate().format(", ")))
    }
}

impl<K, V> RefStore<K,V>
where K: Ref {

    pub fn len(&self) -> usize {
        self.internal.len()
    }

    pub fn keys(&self) -> impl Iterator<Item = K> {
        (0..self.len()).map(|id| K::from(id))
    }

    pub fn last_key(&self) -> Option<K> {
        if self.len() > 0 {
            Some((self.len() -1).into())
        } else {
            None
        }
    }

    pub fn push(&mut self, v: V) -> K
        where V: Eq + Hash + Clone // TODO: remove necessity of clone by storing reference to internal field
    {
        assert!(!self.rev.contains_key(&v));
        let id: K = self.internal.len().into();
        self.rev.insert(v.clone(), id);
        self.internal.push(v);
        id
    }

    pub fn get(&self, k: K) -> &V {
        &self.internal[k.into()]
    }

    pub fn get_ref<W: ?Sized>(&self, v: &W) -> Option<K> where W: Eq + Hash, V: Eq + Hash + Borrow<W> {
        self.rev.get(v).copied()
    }
}

impl<K: Ref,V> Index<K> for RefStore<K,V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index)
    }
}