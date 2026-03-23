use crate::chronicles::Fluent;
use aries::collections::ref_store::{RefPool, RefStore};
use aries::model::lang::Type;
use aries::model::symbols::{ContiguousSymbols, SymId, SymbolTable};
use aries::utils::enumerate;
use core::num::NonZeroU32;
use fixedbitset::FixedBitSet;
use std::collections::HashSet;
use std::fmt::{Display, Error, Formatter};
use std::hash::Hash;
use std::sync::Arc;
use streaming_iterator::StreamingIterator;

/// Compact, numeric representation of a state variable.
///
/// A state variable is typically an s-expression of symbols
/// such as (at bob kitchen) where "at" is a state function and "bob" and "kitchen"
/// are its two parameters.
///
/// TODO: ref to implement with macro.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct SvId(NonZeroU32);

impl SvId {
    pub fn raw(self) -> u32 {
        self.0.get()
    }
    pub fn from_raw(id: u32) -> Self {
        SvId(NonZeroU32::new(id).unwrap())
    }
}

impl From<SvId> for usize {
    fn from(sv: SvId) -> Self {
        (sv.0.get() - 1) as usize
    }
}

impl From<usize> for SvId {
    fn from(i: usize) -> Self {
        let nz = NonZeroU32::new((i + 1) as u32).unwrap();
        SvId(nz)
    }
}

/// Literal: association of a boolean state variable (i.e. predicate) to a boolean value.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct Lit {
    inner: NonZeroU32,
}
impl Lit {
    /// Creates a new (boolean) literal by associating a state variable
    /// with a boolean value.
    pub fn new(sv: SvId, value: bool) -> Lit {
        let sv_usize: usize = sv.into();
        let sv_part: usize = (sv_usize + 1usize) << 1;
        let x = (sv_part as u32) + (value as u32);
        let nz = NonZeroU32::new(x).unwrap();
        Lit { inner: nz }
    }

    /// Returns state variable part of the literal
    pub fn var(self) -> SvId {
        SvId::from((self.inner.get() as usize >> 1) - 1usize)
    }

    /// Returns the value taken by the literal
    pub fn val(self) -> bool {
        (self.inner.get() & 1u32) != 0u32
    }
}
impl std::ops::Not for Lit {
    type Output = Lit;
    fn not(self) -> Self::Output {
        Lit::new(self.var(), !self.val())
    }
}

impl From<Lit> for usize {
    fn from(lit: Lit) -> Self {
        lit.inner.get() as usize - 2usize
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
struct DispSv<'a>(SvId, &'a World);

impl<'a> Display for DispSv<'a> {
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
pub struct World {
    pub table: SymbolTable,
    /// Associates each state variable (represented as an array of symbols [SymId])
    /// to a unique ID (SVId)
    expressions: RefPool<SvId, Box<[SymId]>>,
}

impl World {
    /// Construct a new World (collection of state variables) by building all
    /// state variables that can be constructed from the available state functions.
    ///
    /// Currently, state functions are restricted to take boolean values.
    pub fn new(table: SymbolTable, state_funs: &[Arc<Fluent>]) -> anyhow::Result<Self> {
        let mut s = World {
            table,
            expressions: Default::default(),
        };
        debug_assert_eq!(
            state_funs.iter().map(|p| &p.sym).collect::<HashSet<_>>().len(),
            state_funs.len(),
            "Duplicated predicate"
        );

        for pred in state_funs {
            let mut generators = Vec::with_capacity(1 + pred.argument_types().len());
            let pred_id = pred.sym;
            if pred.return_type() != Type::Bool {
                anyhow::bail!("Non boolean state variable: {}", s.table.symbol(pred_id));
            }

            generators.push(ContiguousSymbols::singleton(pred_id));
            for tpe in pred.argument_types() {
                if let Type::Sym(tpe_id) = tpe {
                    generators.push(s.table.instances_of_type(*tpe_id));
                } else {
                    anyhow::bail!("Non symbolic argument type");
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

    /// Retrieves the ID of a given state variable. Returns None if no such state variable is known.
    pub fn sv_id(&self, sv: &[SymId]) -> Option<SvId> {
        self.expressions.get_ref(sv)
    }

    /// Returns the state variable associated with the given ID
    pub fn sv_of(&self, sv: SvId) -> &[SymId] {
        self.expressions.get(sv)
    }

    pub fn make_new_state(&self) -> State {
        State {
            svs: FixedBitSet::with_capacity(self.expressions.len()),
        }
    }
}

/// State : association of each state variable to its value
#[derive(Clone, Ord, PartialOrd, PartialEq, Eq, Hash)]
pub struct State {
    /// Bitset giving, for each state variable, its boolean value.
    /// The i^th bit of the bitset give the value of the i^th state variables.
    /// The index of the state variable is obtained by converting it into a `usize`.
    svs: FixedBitSet,
}

impl State {
    pub fn num_variables(&self) -> usize {
        self.svs.len()
    }

    pub fn is_set(&self, sv: SvId) -> bool {
        self.svs.contains(sv.into())
    }

    pub fn set_to(&mut self, sv: SvId, value: bool) {
        self.svs.set(sv.into(), value)
    }

    pub fn add(&mut self, sv: SvId) {
        self.set_to(sv, true);
    }

    pub fn del(&mut self, sv: SvId) {
        self.set_to(sv, false);
    }

    pub fn set(&mut self, lit: Lit) {
        self.set_to(lit.var(), lit.val());
    }

    pub fn set_all(&mut self, lits: &[Lit]) {
        lits.iter().for_each(|&l| self.set(l));
    }

    pub fn state_variables(&self) -> impl Iterator<Item = SvId> {
        (0..self.svs.len()).map(SvId::from)
    }

    pub fn literals(&self) -> impl Iterator<Item = Lit> + '_ {
        self.state_variables().map(move |sv| Lit::new(sv, self.is_set(sv)))
    }

    /// Returns all state variables that are true in this state.
    pub fn entailed_variables(&self) -> impl Iterator<Item = SvId> + '_ {
        self.svs.ones().map(SvId::from)
    }

    pub fn entails(&self, lit: Lit) -> bool {
        self.is_set(lit.var()) == lit.val()
    }

    pub fn entails_all(&self, lits: &[Lit]) -> bool {
        lits.iter().all(|&l| self.entails(l))
    }

    pub fn displayable<T, I: Display>(self, desc: &World) -> impl Display + '_ {
        FullState(self, desc)
    }
}

struct FullState<'a>(State, &'a World);

impl<'a> Display for FullState<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        for sv in self.0.entailed_variables() {
            writeln!(f, "{}", DispSv(sv, self.1))?;
        }
        Ok(())
    }
}

/// Representation of classical planning operator
// TODO: should use a small vec to avoid indirection in the common case
// TODO: a common requirement is that effects be applied with delete effects first
//       and add effects second. We should enforce an invariant on the order of effects for this.
pub struct Operator {
    /// SExpression giving the name of the action and its parameters, e.g. (goto bob1 kitchen)
    pub name: Vec<SymId>,
    /// Preconditions of the the operator (might contain negative preconditions.
    pub precond: Vec<Lit>,
    /// Effects of this operator :
    /// - delete effects are literals with a negative value.
    /// - add effects are literals with a positive value.
    pub effects: Vec<Lit>,
}

impl Operator {
    pub fn pre(&self) -> &[Lit] {
        &self.precond
    }

    pub fn eff(&self) -> &[Lit] {
        &self.effects
    }
}

/// Unique numeric identifer of an `Operator`.
/// The correspondence between the id and the operator is done
/// in the `Operators` data structure.
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Op(usize);

impl From<Op> for usize {
    fn from(op: Op) -> Self {
        op.0
    }
}
impl From<usize> for Op {
    fn from(x: usize) -> Self {
        Op(x)
    }
}

#[derive(Default)]
pub struct Operators {
    all: RefStore<Op, Operator>,
    watchers: RefStore<Lit, Vec<Op>>,
    achievers: RefStore<Lit, Vec<Op>>,
}

impl Operators {
    pub fn new() -> Self {
        Default::default()
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
        for &lit in self.all[op].eff() {
            // grow achievers until we have an entry for lit
            while self.achievers.last_key().filter(|&k| k >= lit).is_none() {
                self.achievers.push(Vec::new());
            }
            self.achievers[lit].push(op);
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

    /// Returns all operators that have `lit` as a precondition.
    pub fn dependent_on(&self, lit: Lit) -> &[Op] {
        self.watchers[lit].as_slice()
    }

    /// Returns all operators that have `lit` as an effect.
    pub fn achievers_of(&self, lit: Lit) -> &[Op] {
        self.achievers[lit].as_slice()
    }

    /// An iterator on all Operators in this data structure.
    pub fn iter(&self) -> impl Iterator<Item = Op> + '_ {
        self.all.keys()
    }

    pub fn size(&self) -> usize {
        self.all.len()
    }
}

//#[cfg(test)]
//mod tests {
//    use super::*;
//    use crate::symbols::tests::table;
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
