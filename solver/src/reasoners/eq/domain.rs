use crate::core::literals::Watches;
use crate::core::{IntCst, Lit, SignedVar, VarRef};
use std::collections::HashMap;
use std::ops::RangeInclusive;

#[derive(Clone)]
struct Domain {
    first_value: IntCst,
    value_literals: Vec<Lit>,
}

impl Domain {
    pub fn new() -> Self {
        Domain {
            first_value: 0,
            value_literals: Vec::new(),
        }
    }

    pub fn min(&self) -> IntCst {
        self.first_value
    }

    pub fn max(&self) -> IntCst {
        self.min() + self.value_literals.len() as IntCst - 1
    }

    pub fn bounds(&self) -> RangeInclusive<IntCst> {
        self.min()..=self.max()
    }

    pub fn add_value(&mut self, value: IntCst, lit: Lit) {
        assert!(!self.bounds().contains(&value), "duplicated inclusion");
        assert_eq!(value, self.max() + 1);
        self.value_literals.push(lit);
    }

    pub fn get(&self, value: IntCst) -> Lit {
        debug_assert!(self.bounds().contains(&value));
        self.value_literals[(value - self.first_value) as usize]
    }

    fn values(&self, first: IntCst, last: IntCst) -> &[Lit] {
        let first = (first - self.first_value) as usize;
        let last = (last - self.first_value) as usize;
        &self.value_literals[first..=last]
    }
}

#[derive(Clone, Default)]
pub struct Domains {
    domains: HashMap<VarRef, Domain>,
    eq_watches: Watches<(VarRef, IntCst)>,
    neq_watches: Watches<(VarRef, IntCst)>,
}

impl Domains {
    pub fn has_domain(&self, var: VarRef) -> bool {
        self.domains.contains_key(&var)
    }
    pub fn add_value(&mut self, var: VarRef, value: IntCst, lit: Lit) {
        self.domains
            .entry(var)
            .or_insert_with(|| Domain::new())
            .add_value(value, lit);
        self.eq_watches.add_watch((var, value), lit);
        self.neq_watches.add_watch((var, value), !lit);
    }

    pub fn eq_watches(&self, l: Lit) -> impl Iterator<Item = (VarRef, IntCst)> + '_ {
        self.eq_watches.watches_on(l)
    }

    pub fn neq_watches(&self, l: Lit) -> impl Iterator<Item = (VarRef, IntCst)> + '_ {
        self.neq_watches.watches_on(l)
    }

    pub fn value(&self, v: SignedVar, value: IntCst) -> Lit {
        let dom = &self.domains[&v.variable()];
        if v.is_plus() {
            dom.get(value)
        } else {
            dom.get(-value)
        }
    }
    pub fn values(&self, v: SignedVar, first: IntCst, last: IntCst) -> &[Lit] {
        let dom = &self.domains[&v.variable()];
        if v.is_plus() {
            dom.values(first, last)
        } else {
            dom.values(-last, -first)
        }
    }
}
