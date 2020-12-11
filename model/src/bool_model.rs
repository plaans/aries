use crate::lang::BVar;
use crate::{Label, WriterId};
use aries_backtrack::Q;
use aries_backtrack::{Backtrack, BacktrackWith};
use aries_collections::ref_store::{RefMap, RefVec};
use aries_sat::all::BVar as SatVar;
use aries_sat::all::Lit;

#[derive(Default)]
pub struct BoolModel {
    labels: RefVec<BVar, Label>,
    pub(crate) binding: RefMap<BVar, Lit>,
    pub(crate) values: RefMap<SatVar, bool>,
    pub(crate) trail: Q<(Lit, WriterId)>,
}

impl BoolModel {
    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        self.labels.push(label.into())
    }

    pub fn label(&self, var: BVar) -> Option<&str> {
        self.labels[var].get()
    }

    pub fn bind(&mut self, k: BVar, lit: Lit) {
        assert!(!self.binding.contains(k));
        self.binding.insert(k, lit);
    }

    pub fn literal_of(&self, bvar: BVar) -> Option<Lit> {
        self.binding.get(bvar).copied()
    }

    pub fn variables(&self) -> impl Iterator<Item = BVar> + '_ {
        self.binding.keys()
    }

    /// Returns an iterator on all internal bool variables that have been given a value.
    pub fn bound_sat_variables(&self) -> impl Iterator<Item = (SatVar, bool)> + '_ {
        self.values.entries().map(|(k, v)| (k, *v))
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

impl Clone for BoolModel {
    fn clone(&self) -> Self {
        BoolModel {
            labels: self.labels.clone(),
            binding: self.binding.clone(),
            values: self.values.clone(),
            trail: self.trail.clone(),
        }
    }
}
