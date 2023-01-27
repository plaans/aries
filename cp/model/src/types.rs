use aries_collections::id_map::IdMap;
use aries_collections::ref_store::RefPool;
use aries_utils::input::Sym;
use std::borrow::Borrow;
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::hash::Hash;

// Todo: use Ref
#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq, Hash)]
pub struct TypeId(usize);

impl From<TypeId> for usize {
    fn from(t: TypeId) -> Self {
        t.0
    }
}
impl From<usize> for TypeId {
    fn from(id: usize) -> Self {
        TypeId(id)
    }
}

#[derive(Clone)]
pub struct TypeHierarchy {
    types: RefPool<TypeId, Sym>,
    last_subtype: IdMap<TypeId, TypeId>,
}

#[derive(Debug)]
pub struct UnreachableFromRoot<T>(Vec<(T, Option<T>)>);

impl<T: Debug> Error for UnreachableFromRoot<T> {}

impl<T: Debug> std::fmt::Display for UnreachableFromRoot<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Following types are not reachable from any root type : {:?}", self.0)
    }
}

impl TypeHierarchy {
    /** Constructs the type hierarchy from a set of (type, optional-parent) tuples */
    pub fn new(mut types: Vec<(Sym, Option<Sym>)>) -> Result<Self, UnreachableFromRoot<Sym>> {
        let mut sys = TypeHierarchy {
            types: Default::default(),
            last_subtype: Default::default(),
        };

        let mut trace: Vec<Option<Sym>> = vec![None];

        while !trace.is_empty() {
            let parent = trace.last().unwrap();
            match types.iter().position(|tup| &tup.1 == parent) {
                Some(pos_of_child) => {
                    let child = types.remove(pos_of_child);
                    sys.types.push(child.0.clone());
                    // start looking for its childs
                    trace.push(Some(child.0));
                }
                None => {
                    if let Some(p) = parent {
                        // before removing from trace, record the id of the last child.
                        let parent_id = sys.types.get_ref(p).unwrap();
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

    pub fn id_of<T2: ?Sized>(&self, tpe: &T2) -> Option<TypeId>
    where
        T2: Eq + Hash,
        Sym: Eq + Hash + Borrow<T2>,
    {
        self.types.get_ref(tpe)
    }
    pub fn from_id(&self, tid: TypeId) -> &Sym {
        self.types.get(tid)
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
            ("A".into(), None),
            ("B".into(), None),
            ("A1".into(), Some("A".into())),
            ("A11".into(), Some("A1".into())),
            ("A2".into(), Some("A".into())),
            ("A12".into(), Some("A1".into())),
        ];

        let ts = TypeHierarchy::new(types).unwrap();
        let types = ["A", "B", "A1", "A11", "A12", "A2"];
        let ids: Vec<TypeId> = types.iter().map(|name| ts.id_of(*name).unwrap()).collect();
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
    }
}
