use crate::chronicles::strips::{SymbolTable, SymId, Instances};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Error};
use std::hash::Hash;
use crate::chronicles::enumerate::enumerate;
use streaming_iterator::StreamingIterator;
use crate::chronicles::typesystem::IdVec;
use fixedbitset::FixedBitSet;
use core::num::NonZeroU32;

// TODO: use trait instead of this dummy data structure
#[derive(Debug)]
pub struct PredicateDesc<T,Sym> {
    pub name: Sym,
    pub types: Vec<T>
}


#[derive(Clone, Debug)]
pub struct StateDesc<T,I> {
    pub table: SymbolTable<T,I>,
    expressions: IdVec<SV, Box<[SymId]>>,
    state_vars: HashMap<Box<[SymId]>, SV>

}

#[derive(Copy, Clone,Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct SV(NonZeroU32);

impl SV {
    pub fn raw(self) -> u32 { self.0.get() }
    pub fn from_raw(id: u32) -> Self { SV(NonZeroU32::new(id).unwrap()) }
}

impl Into<usize> for SV {
    fn into(self) -> usize {
        (self.0.get() -1) as usize
    }
}

impl From<usize> for SV {
    fn from(i: usize) -> Self {
        let nz = NonZeroU32::new((i + 1) as u32).unwrap();
        SV(nz)
    }
}



#[derive(Copy, Clone,Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct Lit {
    inner: NonZeroU32
}
impl Lit {
    pub fn new(sv: SV, value: bool) -> Lit {
        let x = (sv.raw() << 1) + (value as u32);
        let nz = NonZeroU32::new(x).unwrap();
        Lit { inner: nz }
    }

    pub fn var(self) -> SV { SV::from_raw(self.inner.get() >> 1) }
    pub fn val(self) -> bool { (self.inner.get() & 1u32) != 0u32 }
}


struct DispSV<'a, T,I>(SV, &'a StateDesc<T,I>);

impl<'a, T,I: Display> Display for DispSV<'a,T,I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "(")?;
        let mut it = self.1.expressions[self.0].iter().peekable();
        while let Some(x) = it.next() {
            write!(f, "{}", self.1.table.symbol(*x))?;
            if it.peek().is_some() {
                write!(f, " ")?;
            }
        }
        write!(f, ")")?;
        Ok(())
    }
}


impl<T, Sym> StateDesc<T, Sym>
{

    pub fn new(table: SymbolTable<T, Sym>, predicates: Vec<PredicateDesc<T,Sym>>) -> Result<Self, String>
        where
            T: Clone + Eq + Hash + Display,
            Sym: Clone + Eq + Hash + Display
    {
        let mut s = StateDesc {
            table,
            expressions: Default::default(),
            state_vars: Default::default()
        };
        assert_eq!(
            predicates.iter().map(|p| &p.name).collect::<HashSet<_>>().len(),
            predicates.len(),
            "Duplicated predicates in {:?}",
            predicates.iter().map(|p| format!("{}", &p.name)).collect::<Vec<_>>()

        );

        for pred in predicates {
            let mut generators = Vec::with_capacity(1 + pred.types.len());
            let pred_id = s.table.id(&pred.name)
                .ok_or(format!("unrecorded pred: {}", pred.name))?;

            generators.push(Instances::singleton(pred_id));
            for tpe in pred.types {
                let tpe_id = s.table.types.id_of(&tpe)
                    .ok_or(format!("unknown type: {}", tpe))?;
                generators.push(s.table.instances_of_type(tpe_id));
            }

            let mut iter = enumerate(generators);
            while let Some(sv) = iter.next() {
                let cpy: Box<[SymId]> = sv.into();
                assert!(!s.state_vars.contains_key(&cpy));
                let id = s.expressions.push(cpy.clone()); // TODO : avoidable copy
                s.state_vars.insert(cpy, id);
            }
        }

        Ok(s)
    }

    pub fn sv_id(&self, sv: &[SymId]) -> Option<SV> {
        self.state_vars.get(sv).copied()
    }

    pub fn make_new_state(&self) -> State {
        State {
            svs: FixedBitSet::with_capacity(self.state_vars.len())
        }
    }
}


#[derive(Clone)]
pub struct State {
    svs: FixedBitSet
}


impl State {

    pub fn is_set(&self, sv: SV) -> bool {
        self.svs.contains(sv.into())
    }

    pub fn set_to(&mut self, sv: SV, value: bool) {
        self.svs.set(sv.into(), value)
    }

    pub fn add(&mut self, sv: SV)  {
        self.set_to(sv, true);
    }

    pub fn del(&mut self, sv: SV) {
        self.set_to(sv, false);
    }

    pub fn state_variables(&self) -> impl Iterator<Item = SV> {
        (0..self.svs.len()).map(|i| SV::from(i))
    }

    pub fn set_svs(&self) -> impl Iterator<Item = SV> + '_ {
        self.svs.ones().map(|i| SV::from(i))
    }

    pub fn displayable<T,I: Display>(self, desc: &StateDesc<T,I>) -> impl Display + '_ {
        FullState(self, desc)
    }
}


struct FullState<'a, T, I>(State, &'a StateDesc<T,I>);

impl<'a,T,I: Display> Display for FullState<'a,T,I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        for sv in self.0.set_svs() {
            write!(f, "{}\n", DispSV(sv, self.1))?;
        }
        Ok(())
    }
}

pub struct Action<T,I> {
    name: I,
    params: Vec<(I, T)>
}

// TODO: should use a small vec to avoid indirection in the common case
pub struct Operator {
    pub name: String,
    pub precond: Vec<Lit>,
    pub effects: Vec<Lit>
}

impl Operator {

    pub fn pre(&self) -> &[Lit] {
        &self.precond
    }

    pub fn eff(&self) -> &[Lit] {
        &self.effects
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::chronicles::strips::tests::table;


    #[test]
    fn state() {
        let table = table();
        let predicates = vec![
            PredicateDesc { name: "at", types: vec!["rover", "location"]},
            PredicateDesc { name: "can_traverse", types: vec!["rover", "location", "location"]}
        ];
        let sd = StateDesc::new(table, predicates).unwrap();
        println!("{:?}", sd);

        let mut s = sd.make_new_state();
        for sv in s.state_variables() {
            s.add(sv);
        }
        println!("{}", FullState(s, &sd));
    }
}

