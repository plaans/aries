use std::collections::{BTreeMap, BTreeSet};

use aries::utils::StreamingIterator;
use planx::FluentId;
use smallvec::SmallVec;
use timelines::{
    IntCst,
    boxes::{BBox, BoxRef},
};

/// An overapproximation of all the values (time, state-variable, value) that may be required in any part of the problem.
#[derive(Debug)]
pub struct RequiredValues {
    values_by_fluent: BTreeMap<FluentId, BBox>,
}

type ParamsInlineVec = SmallVec<[IntCst; 6]>;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct Params(pub ParamsInlineVec);
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct StateVar {
    pub fluent: FluentId,
    pub params: Params,
}

impl RequiredValues {
    pub fn new() -> Self {
        Self {
            values_by_fluent: Default::default(),
        }
    }

    pub fn add(&mut self, fluent: FluentId, value_box: BoxRef<'_>) {
        self.values_by_fluent
            .entry(fluent)
            .or_insert(value_box.to_owned())
            .union(value_box);
    }

    pub fn overlaps(&self, fluent: FluentId, vbox: BoxRef) -> bool {
        self.values_by_fluent
            .get(&fluent)
            .is_some_and(|b| b.as_ref().overlaps(vbox))
    }

    pub fn may_require_value(&self, fluent: FluentId, value: bool) -> bool {
        let value = if value { 1 } else { 0 };
        self.values_by_fluent
            .get(&fluent)
            .is_some_and(|b| b.as_ref().last().unwrap().points().contains(&value))
    }

    pub fn params_box<'a>(&'a self, fluent: FluentId) -> BoxRef<'a> {
        self.values_by_fluent[&fluent].as_ref().drop_head(1).drop_tail(1)
    }

    pub fn params(&self, fluent: FluentId) -> impl StreamingIterator<Item = [IntCst]> {
        self.params_box(fluent).points()
    }

    pub fn state_variables(&self, with_value: impl Fn(IntCst) -> bool) -> BTreeSet<StateVar> {
        let mut all = BTreeSet::new();
        for &f in self.values_by_fluent.keys() {
            let has_matching_value = self.values_by_fluent[&f]
                .as_ref()
                .last()
                .unwrap()
                .points()
                .any(&with_value);
            if has_matching_value {
                self.params(f).for_each(|params| {
                    all.insert(StateVar {
                        fluent: f,
                        params: Params(params.into()),
                    });
                });
            }
        }
        all
    }
}
