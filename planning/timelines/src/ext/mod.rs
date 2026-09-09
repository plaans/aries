pub mod ground;
pub mod lprelax;
mod nonsimple;

use aries_solver::core::IntCst;

pub use nonsimple::collect_nonsimple_conditions_and_effects_to_relax;

pub(crate) type Source = Option<crate::TaskId>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceGrounding(Vec<IntCst>);

impl From<Vec<IntCst>> for SourceGrounding {
    fn from(value: Vec<IntCst>) -> Self {
        Self(value)
    }
}
impl SourceGrounding {
    pub fn get(&self) -> &[IntCst] {
        &self.0
    }
}
impl std::ops::Index<usize> for SourceGrounding {
    type Output = IntCst;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

pub(crate) type SourceGroundingId = usize;
