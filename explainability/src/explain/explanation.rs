use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use aries::core::{Lit, VarRef};
use aries::model::{Label, Model};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Essence(pub BTreeSet<Lit>, pub BTreeSet<Lit>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Substance {
    ModelConstraints(BTreeSet<Lit>),
    CounterExample(BTreeSet<(VarRef, i32)>),
}

pub type EssenceIndex = usize;
pub type SubstanceIndex = usize;
pub type ModelIndex = usize;

pub struct ExplanationFilter {
    pub map: Option<BTreeMap<BTreeSet<ModelIndex>, bool>>,
    pub default: bool,
}
impl ExplanationFilter {
    pub fn includes(&self, combination: BTreeSet<ModelIndex>) -> bool {
        if let Some(map) = &self.map {
            *map.get(&combination).unwrap_or(&self.default)
        } else {
            self.default
        }
    }
}

pub struct Explanation<Lbl: Label> {
    pub models: Vec<Arc<Model<Lbl>>>,
    pub essences: Vec<Essence>,
    pub substances: Vec<Substance>,
    pub table: BTreeMap<EssenceIndex, BTreeMap<SubstanceIndex, BTreeSet<ModelIndex>>>,
    pub filter: ExplanationFilter,
}
