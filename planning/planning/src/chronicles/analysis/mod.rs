use crate::chronicles::Problem;
use std::collections::HashSet;

mod detrimental_supports;
pub mod hierarchy;
mod static_fluents;

pub use detrimental_supports::{CausalSupport, TemplateCondID, TemplateEffID};
pub use static_fluents::is_static;

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
    pub detrimental_supports: HashSet<detrimental_supports::CausalSupport>,
}

pub fn analyse(pb: &Problem) -> Metadata {
    Metadata {
        class: hierarchy::class_of(pb),
        detrimental_supports: detrimental_supports::find_useless_supports(pb),
    }
}
