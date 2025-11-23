use std::collections::BTreeMap;

use aries_sched::boxes::{BBox, BoxRef};

/// An overapproximation of all the values (time, state-variable, value) that may be required in any part of the problem.
#[derive(Debug)]
pub struct RequiredValues {
    values_by_fluent: BTreeMap<String, BBox>,
}

impl RequiredValues {
    pub fn new() -> Self {
        Self {
            values_by_fluent: Default::default(),
        }
    }

    pub fn add(&mut self, fluent: &str, value_box: BoxRef<'_>) {
        if let Some(prev) = self.values_by_fluent.get_mut(fluent) {
            prev.union(value_box);
        } else {
            self.values_by_fluent.insert(fluent.to_string(), value_box.to_owned());
        }
    }

    pub fn overlaps(&self, fluent: &str, vbox: BoxRef) -> bool {
        self.values_by_fluent
            .get(fluent)
            .is_some_and(|b| b.as_ref().overlaps(vbox))
    }
}
