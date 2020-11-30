use crate::lang::*;
use aries_collections::ref_store::RefMap;
use aries_sat::all::Lit;
use std::collections::HashMap;

use crate::backtrack::{Backtrack, BacktrackWith};
use crate::queues::{QReader, Q};
use aries_sat::all::BVar as SatVar;

// struct BidirMulMap<A, B> {
//     lr: HashMap<A, B>,
//     rl:
//
// }

#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct WriterId(u8);
impl WriterId {
    pub fn new(num: impl Into<u8>) -> WriterId {
        WriterId(num.into())
    }
}

#[derive(Default)]
pub struct Model {
    pub bools: BoolModel,
    //ints: IntModel,
}

pub struct ModelEventReaders {
    pub bool_events: QReader<(Lit, WriterId)>,
}

impl Model {
    pub fn bool_event_reader(&self) -> QReader<(Lit, WriterId)> {
        self.bools.trail.reader()
    }

    pub fn readers(&self) -> ModelEventReaders {
        ModelEventReaders {
            bool_events: self.bool_event_reader(),
        }
    }
}

// TODO: account for ints
impl Backtrack for Model {
    fn save_state(&mut self) -> u32 {
        self.bools.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.bools.num_saved()
    }

    fn restore_last(&mut self) {
        self.bools.restore_last()
    }
}

pub struct WModel<'a> {
    model: &'a mut Model,
    token: WriterId,
}

impl<'a> WModel<'a> {
    pub fn set(&mut self, lit: Lit) {
        self.model.bools.set(lit, self.token);
    }
}

#[derive(Default)]
pub struct BoolModel {
    binding: RefMap<BVar, Lit>,
    values: RefMap<SatVar, bool>,
    trail: Q<(Lit, WriterId)>,
}

impl BoolModel {
    pub fn bind(&mut self, k: BVar, lit: Lit) {
        assert!(!self.binding.contains(k));
        self.binding.insert(k, lit);
    }

    pub fn literal_of(&self, bvar: BVar) -> Option<Lit> {
        self.binding.get(bvar).copied()
    }

    pub fn value(&self, lit: Lit) -> Option<bool> {
        self.values
            .get(lit.variable())
            .copied()
            .map(|value| if lit.value() { value } else { !value })
    }

    pub fn value_of(&self, v: BVar) -> Option<bool> {
        self.binding.get(v).and_then(|lit| self.value(*lit))
    }

    pub fn set(&mut self, lit: Lit, writer: WriterId) {
        let var = lit.variable();
        let val = lit.value();
        let prev = self.values.get(var).copied();
        assert_ne!(prev, Some(!val), "Incompatible values");
        if prev.is_none() {
            self.values.insert(var, val);
            self.trail.push((lit, writer));
        } else {
            // no-op
            debug_assert_eq!(prev, Some(val));
        }
    }
}

impl Backtrack for BoolModel {
    fn save_state(&mut self) -> u32 {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        let domains = &mut self.values;
        self.trail.restore_last_with(|(lit, _)| domains.remove(lit.variable()));
    }
}

pub struct IntDomain {
    lb: IntCst,
    ub: IntCst,
}
#[derive(Default)]
pub struct IntModel {
    binding: HashMap<IVar, usize>,
    domains: Vec<Option<IntDomain>>,
}
