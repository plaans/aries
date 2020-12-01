use crate::lang::*;
use aries_collections::ref_store::{RefMap, RefVec};
use aries_sat::all::Lit;
use std::collections::HashMap;

use crate::backtrack::{Backtrack, BacktrackWith};
use crate::queues::{QReader, Q};
use aries_sat::all::BVar as SatVar;

#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct WriterId(u8);
impl WriterId {
    pub fn new(num: impl Into<u8>) -> WriterId {
        WriterId(num.into())
    }
}

// TODO: reorganize
pub struct ModelEvents {
    pub bool_events: QReader<(Lit, WriterId)>,
}

impl Model {
    pub fn bool_event_reader(&self) -> QReader<(Lit, WriterId)> {
        self.bools.trail.reader()
    }

    pub fn readers(&self) -> ModelEvents {
        ModelEvents {
            bool_events: self.bool_event_reader(),
        }
    }

    pub fn writer(&mut self, token: WriterId) -> WModel {
        WModel { model: self, token }
    }
}

// TODO: account for ints
impl Backtrack for Model {
    fn save_state(&mut self) -> u32 {
        let a = self.bools.save_state();
        let b = self.ints.save_state();
        assert_eq!(a, b, "Different number of saved levels");
        a
    }

    fn num_saved(&self) -> u32 {
        assert_eq!(self.bools.num_saved(), self.ints.num_saved());
        self.bools.num_saved()
    }

    fn restore_last(&mut self) {
        self.bools.restore_last();
        self.ints.restore_last();
    }

    fn restore(&mut self, saved_id: u32) {
        self.bools.restore(saved_id);
        self.ints.restore(saved_id);
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

    pub fn set_upper_bound(&mut self, ivar: IV, ub: IntCst) {
        self.model.ints.set_ub(ivar, ub, self.token);
    }
    pub fn set_lower_bound(&mut self, ivar: IV, lb: IntCst) {
        self.model.ints.set_lb(ivar, lb, self.token);
    }
}

#[derive(Default)]
pub struct BoolModel {
    labels: RefVec<BVar, Option<String>>,
    binding: RefMap<BVar, Lit>,
    values: RefMap<SatVar, bool>,
    trail: Q<(Lit, WriterId)>,
}

type Label = String;

impl BoolModel {
    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        let label = label.into();
        let label = if label.len() == 0 { None } else { Some(label) };
        self.labels.push(label)
    }

    pub fn label(&self, var: BVar) -> Option<&Label> {
        self.labels[var].as_ref()
    }

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
    pub lb: IntCst,
    pub ub: IntCst,
}
impl IntDomain {
    pub fn new(lb: IntCst, ub: IntCst) -> IntDomain {
        IntDomain { lb, ub }
    }
}
pub struct VarEvent {
    pub var: IV,
    pub ev: DomEvent,
}
pub enum DomEvent {
    NewLB { prev: IntCst, new: IntCst },
    NewUB { prev: IntCst, new: IntCst },
}

type IV = crate::lang::IVar;

#[derive(Default)]
pub struct IntModel {
    labels: RefVec<IV, Option<String>>,
    domains: RefVec<IV, IntDomain>,
    trail: Q<(VarEvent, WriterId)>,
}

impl IntModel {
    pub fn new() -> IntModel {
        IntModel {
            labels: Default::default(),
            domains: Default::default(),
            trail: Default::default(),
        }
    }

    pub fn new_ivar<L: Into<Label>>(&mut self, lb: IntCst, ub: IntCst, label: L) -> IVar {
        let label = label.into();
        let label = if label.len() == 0 { None } else { Some(label) };
        // self.ints.push(IntVarDesc::new(lb, ub, label));
        let id1 = self.labels.push(label);
        let id2 = self.domains.push(IntDomain::new(lb, ub));
        debug_assert_eq!(id1, id2);
        id1
    }

    pub fn label(&self, var: IV) -> Option<&Label> {
        self.labels[var].as_ref()
    }

    pub fn domain_of(&self, var: IV) -> &IntDomain {
        &self.domains[var]
    }

    fn dom_mut(&mut self, var: IV) -> &mut IntDomain {
        &mut self.domains[var]
    }

    pub fn set_lb(&mut self, var: IV, lb: IntCst, writer: WriterId) {
        let dom = self.dom_mut(var);
        let prev = dom.lb;
        if prev < lb {
            dom.lb = lb;
            let event = VarEvent {
                var,
                ev: DomEvent::NewLB { prev, new: lb },
            };
            self.trail.push((event, writer));
        }
    }

    pub fn set_ub(&mut self, var: IV, ub: IntCst, writer: WriterId) {
        let dom = self.dom_mut(var);
        let prev = dom.ub;
        if prev > ub {
            dom.ub = ub;
            let event = VarEvent {
                var,
                ev: DomEvent::NewUB { prev, new: ub },
            };
            self.trail.push((event, writer));
        }
    }

    fn undo_event(domains: &mut RefVec<IV, IntDomain>, ev: VarEvent) {
        let dom = &mut domains[ev.var];
        match ev.ev {
            DomEvent::NewLB { prev, new } => {
                debug_assert_eq!(dom.lb, new);
                dom.lb = prev;
            }
            DomEvent::NewUB { prev, new } => {
                debug_assert_eq!(dom.ub, new);
                dom.ub = prev;
            }
        }
    }
}

impl Backtrack for IntModel {
    fn save_state(&mut self) -> u32 {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        let domains = &mut self.domains;
        self.trail.restore_last_with(|(ev, _)| Self::undo_event(domains, ev));
    }

    fn restore(&mut self, saved_id: u32) {
        let domains = &mut self.domains;
        self.trail
            .restore_with(saved_id, |(ev, _)| Self::undo_event(domains, ev));
    }
}
