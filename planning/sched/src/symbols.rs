use std::{borrow::Borrow, collections::BTreeMap, fmt::Debug};

use aries::core::IntCst;

use crate::SymAtom;

#[derive(Clone)]
pub struct Range {
    pub first: IntCst,
    pub last: IntCst,
}
impl Debug for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.first, self.last)
    }
}

type Sym = crate::Sym;

#[derive(Clone, Debug)]
pub struct ObjectEncoding {
    types: BTreeMap<Sym, Range>,
    objects: BTreeMap<Sym, IntCst>,
}

impl ObjectEncoding {
    pub fn domain_of_type<Q>(&self, key: &Q) -> Option<Range>
    where
        Sym: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.types.get(key).cloned()
    }

    pub fn object_id<Q>(&self, key: &Q) -> Option<IntCst>
    where
        Sym: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.objects.get(key).copied()
    }

    pub fn object_atom<Q>(&self, key: &Q) -> Option<SymAtom>
    where
        Sym: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.object_id(key).map(SymAtom::from)
    }

    pub fn build(top: Sym, children: impl Fn(&Sym) -> Vec<Sym>, objects: impl Fn(&Sym) -> Vec<Sym>) -> ObjectEncoding {
        let mut objs = ObjectEncoding {
            types: Default::default(),
            objects: Default::default(),
        };
        objs.process(top, &children, &objects);
        objs
    }
    fn process(&mut self, curr: Sym, children: &dyn Fn(&Sym) -> Vec<Sym>, objects: &dyn Fn(&Sym) -> Vec<Sym>) {
        assert!(!self.types.contains_key(&curr));

        let first = self.next_object_id();
        for o in objects(&curr) {
            assert!(!self.objects.contains_key(&o));
            self.objects.insert(o, self.next_object_id());
        }
        for subtype in children(&curr) {
            self.process(subtype, children, objects);
        }

        let last = self.next_object_id() - 1;
        self.types.insert(curr, Range { first, last });
    }

    fn next_object_id(&self) -> IntCst {
        self.objects.len() as IntCst
    }
}
