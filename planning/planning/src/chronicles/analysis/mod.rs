use crate::chronicles::{ChronicleLabel, Problem};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod detrimental_supports;
mod features;
pub mod fluent_hierarchy;
pub mod hierarchy;
mod static_fluents;

pub use crate::chronicles::analysis::features::*;
use crate::chronicles::preprocessing::action_rolling::RollCompilation;
pub use detrimental_supports::{CausalSupport, CondOrigin, TemplateCondID, TemplateEffID};
pub use static_fluents::is_static;

pub type TemplateID = usize;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ProblemClass {
    FlatNoTemplates,
    FlatTemplates,
    HierarchicalRecursive,
    HierarchicalNonRecursive,
}

impl ProblemClass {
    pub fn is_hierarchical(&self) -> bool {
        matches!(
            self,
            ProblemClass::HierarchicalNonRecursive | ProblemClass::HierarchicalRecursive
        )
    }
}

/// A set of metadata of a problem, typically gather through the analysis of the unbounded problem
pub struct Metadata {
    pub class: ProblemClass,
    pub features: FeatureSet,
    pub detrimental_supports: HashSet<CausalSupport>,
    pub action_hierarchy: HashMap<TemplateID, usize>,
    /// If the template is a rolled-up action, associates the corresponding compilation to allow unrolling it
    pub action_rolling: HashMap<TemplateID, Arc<RollCompilation>>,
}

pub fn analyse(pb: &Problem) -> Metadata {
    let mut action_rolling = HashMap::new();
    for template_id in 0..pb.templates.len() {
        if let ChronicleLabel::RolledAction(_, compil) = &pb.templates[template_id].label {
            action_rolling.insert(template_id, compil.clone());
        }
    }
    Metadata {
        class: hierarchy::class_of(pb),
        features: features::features(pb),
        detrimental_supports: detrimental_supports::find_useless_supports(pb),
        action_hierarchy: fluent_hierarchy::hierarchy(pb),
        action_rolling,
    }
}
