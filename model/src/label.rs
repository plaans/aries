use crate::lang::VarRef;
use aries_collections::ref_store::RefMap;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

/// Trait requiring the minimum capabilities for a type to serve as the label of variables.
pub trait Label: Debug + Clone + Eq + PartialEq + Hash + Send + Sync + 'static {}
impl<T> Label for T where T: Debug + Clone + Eq + PartialEq + Hash + Send + Sync + 'static {}

#[derive(Clone)]
pub struct VariableLabels<Lbl> {
    labels: RefMap<VarRef, Arc<Lbl>>,
    labeled_variables: HashMap<Arc<Lbl>, VarRef>,
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
        assert!(
            !self.labeled_variables.contains_key(label.as_ref()),
            "Already a variable with label {:?}",
            &label
        );
        self.labeled_variables.insert(label, var);
    }

    pub fn get_var(&self, label: &Lbl) -> Option<VarRef>
    where
        Lbl: Label,
    {
        self.labeled_variables.get(label).copied()
    }
}

impl<Lbl> Default for VariableLabels<Lbl> {
    fn default() -> Self {
        Self::new()
    }
}
