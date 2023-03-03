use aries::collections::ref_store::RefMap;
use aries::core::*;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::sync::Arc;

/// Trait requiring the minimum capabilities for a type to serve as the label of variables.
pub trait Label: Display + Debug + Clone + Eq + PartialEq + Hash + Send + Sync + 'static {}
impl<T> Label for T where T: Display + Debug + Clone + Eq + PartialEq + Hash + Send + Sync + 'static {}

#[derive(Clone)]
pub struct VariableLabels<Lbl> {
    labels: RefMap<VarRef, Arc<Lbl>>,
    labeled_variables: HashMap<Arc<Lbl>, Vec<VarRef>>,
}

impl<Lbl> VariableLabels<Lbl> {
    pub fn new() -> Self {
        Self {
            labels: Default::default(),
            labeled_variables: Default::default(),
        }
    }
    pub fn get(&self, var: VarRef) -> Option<&Lbl> {
        self.labels.get(var).map(|l| l.as_ref())
    }

    pub fn insert(&mut self, var: VarRef, label: impl Into<Arc<Lbl>>)
    where
        Lbl: Label,
    {
        let label = label.into();
        self.labels.insert(var, label.clone());
        let vars = self
            .labeled_variables
            .entry(label)
            .or_insert_with(|| Vec::with_capacity(1));
        vars.push(var);
    }

    pub fn variables_with_label(&self, label: &Lbl) -> &[VarRef]
    where
        Lbl: Label,
    {
        self.labeled_variables.get(label).map(|v| v.as_ref()).unwrap_or(&[])
    }
}

impl<Lbl> Default for VariableLabels<Lbl> {
    fn default() -> Self {
        Self::new()
    }
}
