use crate::all::BVar;
use aries_collections::heap::IdxHeap;
use aries_collections::index_map::{IndexMap, ToIndex};
use aries_collections::Next;
use std::ops::Index;

pub struct HeurParams {
    var_inc: f64,
    var_decay: f64,
}
impl Default for HeurParams {
    fn default() -> Self {
        HeurParams {
            var_inc: 1_f64,
            var_decay: 0.95_f64,
        }
    }
}

pub struct Heur {
    params: HeurParams,
    activities: IndexMap<BVar, f64>,
    heap: IdxHeap<BVar>,
}

impl Heur {
    pub fn init(num_vars: u32, params: HeurParams) -> Self {
        let mut h = Heur {
            params,
            activities: IndexMap::new(num_vars as usize + 2, 1_f64),
            heap: IdxHeap::new_with_capacity(num_vars as usize + 2),
        };
        for v in BVar::first(num_vars as usize) {
            h.heap.insert(v, |a, b| a.to_index() < b.to_index());
        }
        h
    }
    fn by_max<'a, Arr: Index<BVar, Output = f64>>(h: &'a Arr) -> impl Fn(BVar, BVar) -> bool + 'a {
        move |a, b| h[a] > h[b]
    }

    pub fn next_var(&mut self) -> Option<BVar> {
        let acts = &self.activities;
        self.heap.pop(Heur::by_max(acts))
    }

    pub fn var_insert(&mut self, var: BVar) {
        let acts = &self.activities;
        self.heap.insert_or_update(var, Heur::by_max(acts))
    }

    pub fn var_bump_activity(&mut self, var: BVar) {
        let a = &mut self.activities[var];
        *a += self.params.var_inc;
        if *a > 1e100_f64 {
            self.var_rescale_activity()
        }
        let acts = &self.activities;
        let heap = &mut self.heap;
        if heap.contains(var) {
            heap.update(var, Heur::by_max(acts));
        }
    }

    pub fn decay_activities(&mut self) {
        self.params.var_inc /= self.params.var_decay;
    }

    fn var_rescale_activity(&mut self) {
        self.activities.values_mut().for_each(|v| *v *= 1e-100_f64);
        self.params.var_inc *= 1e-100_f64;
    }
}
