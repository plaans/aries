use crate::backtrack::{Backtrack, BacktrackWith};
use crate::model::lang::BVar;
use crate::model::WriterId;
use crate::queues::Q;
use aries_collections::ref_store::{RefMap, RefVec};
use aries_sat::all::BVar as SatVar;
use aries_sat::all::Lit;

type Label = String;

#[derive(Default)]
pub struct BoolModel {
    labels: RefVec<BVar, Option<Label>>,
    binding: RefMap<BVar, Lit>,
    values: RefMap<SatVar, bool>,
    pub(crate) trail: Q<(Lit, WriterId)>,
}

impl BoolModel {
    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        let label = label.into();
        let label = if label.is_empty() { None } else { Some(label) };
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
