pub mod grounding;
mod nonsimple;
#[allow(dead_code)]
pub mod transitions;

pub use nonsimple::collect_nonsimple_conditions_and_effects_to_relax;

pub(crate) type Source = Option<crate::TaskId>;
