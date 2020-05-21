use crate::planning::chronicles::{StateVar, Type};
use crate::planning::ref_store::{RefPool, RefStore};
use crate::planning::symbols::{Instances, SymId, SymbolTable};
use crate::planning::utils::enumerate;
use core::num::NonZeroU32;
use fixedbitset::FixedBitSet;
use std::collections::HashSet;
use std::fmt::{Display, Error, Formatter};
use std::hash::Hash;
use streaming_iterator::StreamingIterator;

/// Compact, numeric representation of a state variable.
///
/// A state variable is typically an s-expression of symbols
/// such as (at bob kitchen) where "at" is a state function and "bob" and "kitchen"
/// are its two parameters.
///
/// TODO: ref to implement with macro.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct SVId(NonZeroU32);

impl SVId {
    pub fn raw(self) -> u32 {
        self.0.get()
    }
    pub fn from_raw(id: u32) -> Self {
        SVId(NonZeroU32::new(id).unwrap())
    }
}

impl Into<usize> for SVId {
    fn into(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

impl From<usize> for SVId {
    fn from(i: usize) -> Self {
        let nz = NonZeroU32::new((i + 1) as u32).unwrap();
        SVId(nz)
    }
}

/// Association of a boolean state variable (i.e. predicate) to a boolean value.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct Lit {
    inner: NonZeroU32,
}
impl Lit {
    /// Creates a new (boolean) literal by associating a state variable
    /// with a boolean value.
    pub fn new(sv: SVId, value: bool) -> Lit {
        let sv_usize: usize = sv.into();
        let sv_part: usize = (sv_usize + 1usize) << 1;
        let x = (sv_part as u32) + (value as u32);
        let nz = NonZeroU32::new(x).unwrap();
        Lit { inner: nz }
    }

    /// Returns state variable part of the literal
    pub fn var(self) -> SVId {
        SVId::from((self.inner.get() as usize >> 1) - 1usize)
    }

    /// Returns the value taken by the literal
    pub fn val(self) -> bool {
        (self.inner.get() & 1u32) != 0u32
    }
}
impl Into<usize> for Lit {
    fn into(self) -> usize {
        self.inner.get() as usize - 2usize
    }
}
impl From<usize> for Lit {
    fn from(x: usize) -> Self {
        Lit {
            inner: NonZeroU32::new(x as u32 + 2u32).unwrap(),
        }
    }
}

/// Composition of a state variable ID and its defining world.
/// IT allows looking up information in the world to
/// implements things such as Display.
struct DispSV<'a, T, I>(SVId, &'a World<T, I>);

impl<'a, T, I: Display> Display for DispSV<'a, T, I> {
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

/// Keeps track of all state variables that can appear in a state.
#[derive(Clone, Debug)]
pub struct World<T, I> {
    pub table: SymbolTable<T, I>,
    /// Associates each state variable (represented as an array of symbols [SymId]
    /// to a unique ID (SVId)
    expressions: RefPool<SVId, Box<[SymId]>>,
}

impl<T, Sym> World<T, Sym> {
    pub fn new(table: SymbolTable<T, Sym>, predicates: &[StateVar]) -> Result<Self, String>
    where
        T: Clone + Eq + Hash + Display,
        Sym: Clone + Eq + Hash + Display,
    {
        let mut s = World {
            table,
            expressions: Default::default(),
        };
        debug_assert_eq!(
            predicates
                .iter()
                .map(|p| &p.sym)
                .collect::<HashSet<_>>()
                .len(),
            predicates.len(),
            "Duplicated predicate"
        );

        for pred in predicates {
            let mut generators = Vec::with_capacity(1 + pred.argument_types().len());
            let pred_id = pred.sym;
            if pred.return_type() != Type::Boolean {
                return Err(format!(
                    "Non boolean state variable: {}",
                    s.table.symbol(pred_id)
                ));
            }

            generators.push(Instances::singleton(pred_id));
            for tpe in pred.argument_types() {
                if let Type::Symbolic(tpe_id) = tpe {
                    generators.push(s.table.instances_of_type(*tpe_id));
                } else {
                    return Err("Non symbolic argument type".to_string());
                }
            }

            let mut iter = enumerate(generators);
            while let Some(sv) = iter.next() {
                let cpy: Box<[SymId]> = sv.into();
                assert!(s.expressions.get_ref(&cpy).is_none());
                s.expressions.push(cpy);
            }
        }

        Ok(s)
    }

    pub fn sv_id(&self, sv: &[SymId]) -> Option<SVId> {
        self.expressions.get_ref(sv)
    }

    pub fn sv_of(&self, sv: SVId) -> &[SymId] {
        self.expressions.get(sv)
    }

    pub fn make_new_state(&self) -> State {
        State {
            svs: FixedBitSet::with_capacity(self.expressions.len()),
        }
    }
}

#[derive(Clone, Ord, PartialOrd, PartialEq, Eq, Hash)]
pub struct State {
    svs: FixedBitSet,
}

impl State {
    pub fn size(&self) -> usize {
        self.svs.len()
    }

    pub fn is_set(&self, sv: SVId) -> bool {
        self.svs.contains(sv.into())
    }

    pub fn set_to(&mut self, sv: SVId, value: bool) {
        self.svs.set(sv.into(), value)
    }

    pub fn add(&mut self, sv: SVId) {
        self.set_to(sv, true);
    }

    pub fn del(&mut self, sv: SVId) {
        self.set_to(sv, false);
    }

    pub fn set(&mut self, lit: Lit) {
        self.set_to(lit.var(), lit.val());
    }

    pub fn set_all(&mut self, lits: &[Lit]) {
        lits.iter().for_each(|&l| self.set(l));
    }

    pub fn state_variables(&self) -> impl Iterator<Item = SVId> {
        (0..self.svs.len()).map(SVId::from)
    }

    pub fn literals(&self) -> impl Iterator<Item = Lit> + '_ {
        self.state_variables()
            .map(move |sv| Lit::new(sv, self.is_set(sv)))
    }

    pub fn set_svs(&self) -> impl Iterator<Item = SVId> + '_ {
        self.svs.ones().map(SVId::from)
    }

    pub fn entails(&self, lit: Lit) -> bool {
        self.is_set(lit.var()) == lit.val()
    }

    pub fn entails_all(&self, lits: &[Lit]) -> bool {
        lits.iter().all(|&l| self.entails(l))
    }

    pub fn displayable<T, I: Display>(self, desc: &World<T, I>) -> impl Display + '_ {
        FullState(self, desc)
    }
}

struct FullState<'a, T, I>(State, &'a World<T, I>);

impl<'a, T, I: Display> Display for FullState<'a, T, I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        for sv in self.0.set_svs() {
            writeln!(f, "{}", DispSV(sv, self.1))?;
        }
        Ok(())
    }
}

// TODO: should use a small vec to avoid indirection in the common case
pub struct Operator {
    pub name: Vec<SymId>,
    pub precond: Vec<Lit>,
    pub effects: Vec<Lit>,
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Op(usize);

impl Into<usize> for Op {
    fn into(self) -> usize {
        self.0
    }
}
impl From<usize> for Op {
    fn from(x: usize) -> Self {
        Op(x)
    }
}

pub struct Operators {
    all: RefStore<Op, Operator>,
    watchers: RefStore<Lit, Vec<Op>>,
}

impl Operators {
    pub fn new() -> Self {
        Operators {
            all: RefStore::new(),
            watchers: RefStore::new(),
        }
    }
    pub fn push(&mut self, o: Operator) -> Op {
        let op = self.all.push(o);
        for &lit in self.all[op].pre() {
            // grow watchers until we have an entry for lit
            while self.watchers.last_key().filter(|&k| k >= lit).is_none() {
                self.watchers.push(Vec::new());
            }
            self.watchers[lit].push(op);
        }
        op
    }

    pub fn preconditions(&self, op: Op) -> &[Lit] {
        self.all[op].pre()
    }

    pub fn effects(&self, op: Op) -> &[Lit] {
        self.all[op].eff()
    }

    pub fn name(&self, op: Op) -> &[SymId] {
        &self.all[op].name
    }

    pub fn dependent_on(&self, lit: Lit) -> &[Op] {
        self.watchers[lit].as_slice()
    }

    pub fn iter(&self) -> impl Iterator<Item = Op> {
        self.all.keys()
    }

    pub fn size(&self) -> usize {
        self.all.len()
    }
}

impl Operator {
    pub fn pre(&self) -> &[Lit] {
        &self.precond
    }

    pub fn eff(&self) -> &[Lit] {
        &self.effects
    }
}

//#[cfg(test)]
//mod tests {
//    use super::*;
//    use crate::planning::symbols::tests::table;
//
//
//    #[test]
//    fn state() {
//        let table = table();
//        let predicates = vec![
//            PredicateDesc { name: "at", types: vec!["rover", "location"]},
//            PredicateDesc { name: "can_traverse", types: vec!["rover", "location", "location"]}
//        ];
//        let sd = StateDesc::new(table, predicates).unwrap();
//        println!("{:?}", sd);
//
//        let mut s = sd.make_new_state();
//        for sv in s.state_variables() {
//            s.add(sv);
//        }
//        println!("{}", FullState(s, &sd));
//    }
//}
//
