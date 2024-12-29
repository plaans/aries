use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use aries::core::Lit;
use aries::model::{Label, Model};

pub struct ExplEssence(pub BTreeSet<Lit>, pub BTreeSet<Lit>);

pub enum ExplSubstance {
    Modelling(BTreeSet<Lit>),
    CounterExample(BTreeSet<Lit>),
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
    pub essences: Vec<ExplEssence>,
    pub substances: Vec<ExplSubstance>,
    pub table: BTreeMap<(EssenceIndex, SubstanceIndex), BTreeSet<ModelIndex>>,
    pub filter: ExplanationFilter,
}