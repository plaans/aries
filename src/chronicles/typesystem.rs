use std::collections::HashMap;
use crate::collection::id_map::IdMap;
use std::hash::Hash;
use std::marker::PhantomData;


#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq, Hash)]
pub struct TypeId(usize);

impl Into<usize> for TypeId {
    fn into(self) -> usize {
        self.0
    }
}
impl From<usize> for TypeId {
    fn from(id: usize) -> Self {
        TypeId(id)
    }
}

#[derive(Debug, Clone)]
struct IdVec<Key,Val> {
    internal: Vec<Val>,
    phantom: PhantomData<Key>
}
impl<K,V> Default for IdVec<K,V> {
    fn default() -> Self {
        IdVec { internal: Default::default(), phantom: Default::default() }
    }
}

impl<K : Into<usize> + From<usize>, V> IdVec<K,V> {

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

    pub fn push(&mut self, v: V) -> K {
        let id = self.internal.len();
        self.internal.push(v);
        id.into()
    }

    pub fn update(&mut self, k: K, v: V) {
        let index: usize = k.into();
        self.internal[index] = v;
    }

    pub fn get(&self, k: K) -> &V {
        &self.internal[k.into()]
    }

}

#[derive(Clone)]
pub struct TypeHierarchy<T> {
    types: IdVec<TypeId, T>,
    ids: HashMap<T, TypeId>,
    last_subtype: IdMap<TypeId, TypeId>
}

#[derive(Debug)]
pub struct UnreachableFromRoot<T>(Vec<(T,Option<T>)>);

impl<T : Clone + Eq + Hash> TypeHierarchy<T> {

    /** Constructs the type hiearchy from a set of (type, optional-parent) tuples */
    pub fn new(mut types: Vec<(T, Option<T>)>) -> Result<Self, UnreachableFromRoot<T>> {
        let mut sys = TypeHierarchy {
            types: Default::default(),
            ids: Default::default(),
            last_subtype: Default::default()
        };

        let mut trace: Vec<Option<T>> = Vec::new();
        trace.push(None);

        while !trace.is_empty() {
            let parent = trace.last().unwrap();
            match types.iter().position(|tup| &tup.1 == parent) {
                Some(pos_of_child) => {
                    let child = types.remove(pos_of_child);
                    let type_id = sys.types.push(child.0.clone());
                    sys.ids.insert(child.0.clone(), type_id);
                    // start looking for its childs
                    trace.push(Some(child.0));
                },
                None => {
                    if let Some(p) = parent {
                        // before removing from trace, record the id of the last child.
                        let parent_id = sys.ids[p];
                        sys.last_subtype.insert(parent_id, sys.types.last_key().unwrap());
                    }
                    trace.pop();
                }
            }
        }
        if types.is_empty() {
            Result::Ok(sys)
        } else {
            Result::Err(UnreachableFromRoot(types))
        }
    }


    pub fn id_of(&self, tpe: &T) -> Option<TypeId> {
        self.ids.get(tpe).copied()
    }

    pub fn is_subtype(&self, tpe: TypeId, possible_subtype: TypeId) -> bool {
        tpe <= possible_subtype && possible_subtype <= self.last_subtype[tpe]
    }

    pub fn last_subtype(&self, tpe: TypeId) -> TypeId {
        let sub = self.last_subtype[tpe];
        debug_assert!(self.is_subtype(tpe, sub));
        sub
    }

    /// Iterator on all Types by increasing usize value
    pub fn types(&self) -> impl Iterator<Item = TypeId> {
        self.types.keys()
    }
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn type_system() {

        let types = vec![
            ("A", None),
            ("B", None),
            ("A1", Some("A")),
            ("A11", Some("A1")),
            ("A2", Some("A")),
            ("A12", Some("A1"))
        ];

        let ts = TypeHierarchy::new(types).unwrap();
        let types = ["A", "B", "A1", "A11", "A12", "A2"];
        let ids: Vec<TypeId> = types.iter().map(|name| ts.id_of(name).unwrap()).collect();
        if let [a, b, a1, a11, a12, a2] = *ids {
            assert!(ts.is_subtype(a, a));
            assert!(ts.is_subtype(a, a1));
            assert!(ts.is_subtype(a, a11));
            assert!(ts.is_subtype(a, a12));
            assert!(ts.is_subtype(a, a2));

            assert!(ts.is_subtype(a1, a1));
            assert!(ts.is_subtype(a1, a11));
            assert!(ts.is_subtype(a1, a12));
            assert!(!ts.is_subtype(a1, a));

            assert!(!ts.is_subtype(a, b));
            assert!(!ts.is_subtype(b, a));
        } else {
            panic!();
        }

//        for tid in ts.types() {
//            println!("{:?} <- {}", tid, ts.types.get(tid))
//        }

    }

}