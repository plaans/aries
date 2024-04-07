use crate::chronicles::Problem;
use std::collections::{HashMap, HashSet};

mod detrimental_supports;
pub mod fluent_hierarchy;
pub mod hierarchy;
mod static_fluents;

pub use detrimental_supports::{CausalSupport, TemplateCondID, TemplateEffID};
pub use static_fluents::is_static;

pub type TemplateID = usize;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ProblemClass {
    FlatNoTemplates,
    FlatTemplates,
    HierarchicalRecursive,
    HierarchicalNonRecursive,
}

/// A set of metadata of a problem, typically gather through the analysis of the unbounded problem
pub struct Metadata {
    pub class: ProblemClass,
    pub detrimental_supports: HashSet<CausalSupport>,
    pub action_hierarchy: HashMap<TemplateID, usize>,
}

pub fn analyse(pb: &Problem) -> Metadata {
    Metadata {
        class: hierarchy::class_of(pb),
        detrimental_supports: detrimental_supports::find_useless_supports(pb),
        action_hierarchy: fluent_hierarchy::hierarchy(pb),
    }
}
