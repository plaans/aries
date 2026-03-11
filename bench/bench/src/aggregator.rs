use std::{
    collections::BTreeMap,
    ops::{AddAssign, DivAssign},
};

pub fn sum<Measure, Key: Ord + Clone, T: AddAssign<T>>(
    measures: impl IntoIterator<Item = Measure>,
    kv: impl Fn(Measure) -> (Key, T),
) -> BTreeMap<Key, T> {
    Sum.aggregate(measures, kv)
}

pub fn avg<Measure, Key: Ord + Clone, T: AddAssign<T> + DivAssign<f64>>(
    measures: impl IntoIterator<Item = Measure>,
    kv: impl Fn(Measure) -> (Key, T),
) -> BTreeMap<Key, T> {
    Avg.aggregate(measures, kv)
}

pub trait Aggregator<T> {
    fn aggregate<Measure, Key: Ord + Clone>(
        &self,
        measures: impl IntoIterator<Item = Measure>,
        kv: impl Fn(Measure) -> (Key, T),
    ) -> BTreeMap<Key, T>;
}

pub struct Sum;

impl<T: AddAssign<T>> Aggregator<T> for Sum {
    fn aggregate<Measure, Key: Ord>(
        &self,
        measures: impl IntoIterator<Item = Measure>,
        kv: impl Fn(Measure) -> (Key, T),
    ) -> BTreeMap<Key, T> {
        let mut results = BTreeMap::default();
        for measure in measures {
            let (k, v) = kv(measure);
            if let Some(prev) = results.get_mut(&k) {
                *prev += v;
            } else {
                results.insert(k, v);
            }
        }
        results
    }
}

pub struct Avg;

impl<T: AddAssign<T> + DivAssign<f64>> Aggregator<T> for Avg {
    fn aggregate<Measure, Key: Ord + Clone>(
        &self,
        measures: impl IntoIterator<Item = Measure>,
        kv: impl Fn(Measure) -> (Key, T),
    ) -> BTreeMap<Key, T> {
        let mut counts: BTreeMap<_, i32> = BTreeMap::new();
        let mut results = BTreeMap::new();
        for measure in measures {
            let (k, v) = kv(measure);
            *counts.entry(k.clone()).or_default() += 1;
            if let Some(prev) = results.get_mut(&k) {
                *prev += v;
            } else {
                results.insert(k, v);
            }
        }
        for (k, v) in results.iter_mut() {
            *v /= counts[k] as f64;
        }
        results
    }
}
