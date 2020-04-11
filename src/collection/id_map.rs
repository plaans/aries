
use vec_map::VecMap;
use std::convert::TryFrom;
use std::ops::Index;

#[derive(Debug)]
pub struct IdMap<K, V> {
    internal: VecMap<V>,
    phantom: std::marker::PhantomData<K>
}

impl<K: Into<usize>, V> IdMap<K,V> {

    pub fn new() -> Self {
        IdMap {
            internal: Default::default(),
            phantom: std::marker::PhantomData
        }
    }

    pub fn contains_key(&self, k: K) -> bool {
        self.internal.contains_key(k.into())
    }

    pub fn remove(&mut self, k: K) -> Option<V> {
        self.internal.remove(k.into())
    }

    pub fn insert(&mut self, k: K, v: V) {
        self.internal.insert(k.into(), v);
    }

    pub fn get(&self, k: K) -> Option<&V> {
        self.internal.get(k.into())
    }
    pub fn get_with_default(&self, k: K, default: V) -> V where V: Copy {
        *self.internal.get(k.into()).unwrap_or(&default)
    }

    pub fn get_mut(&mut self, k: K) -> Option<&mut V> {
        self.internal.get_mut(k.into())
    }

    pub fn map<V2>(&self, f: &dyn Fn(&V) -> V2) -> IdMap<K,V2> {
        let mut map2 = IdMap::new();
        // todo: use self.internal.into_iter()
        for k in self.internal.keys() {
            let v = self.internal.get(k).unwrap();
            map2.internal.insert(k, f(v));
        }
        map2
    }

    pub fn keys_vec(&self) -> Vec<K>
        where K: TryFrom<usize> {
        let mut v = Vec::with_capacity(self.internal.len());
        for ki in self.internal.keys() {
            match K::try_from(ki) {
                Ok(k) => v.push(k),
                Err(_) => panic!("Could not reconstruct a key from its usize representation"),
            }
        }
        v
    }

    pub fn items_vec(&self) -> Vec<(K,&V)>
        where K: TryFrom<usize> {
        let mut v = Vec::with_capacity(self.internal.len());
        for ki in self.internal.keys() {
            match K::try_from(ki) {
                Ok(k) => v.push((k, self.internal.get(ki).unwrap())),
                Err(_) => panic!("Could not reconstruct a key from its usize representation"),
            }

        }
        v
    }
}


impl<K: Into<usize>,V> Index<K> for IdMap<K,V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index).expect("Key not in map")
    }
}
// Remove because it would rash when trying to add a new entry into the Map
//impl<K: Into<usize>,V> IndexMut<K> for IdMap<K,V> {
//    fn index_mut(&mut self, index: K) -> &mut Self::Output {
//        self.get_mut(index).unwrap()
//    }
//}