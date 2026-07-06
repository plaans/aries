use aries_solver::prelude::IntCst;

use crate::ext::{Grounding, GroundingFlatId};

use std::ops::Index;

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct SourceGroundingFlatId(pub GroundingFlatId);
#[derive(Debug, Clone)]
pub struct SourceGrounding(Grounding);

impl SourceGrounding {
    pub fn to_flat_id(&self, ranges: &[(IntCst, IntCst)]) -> SourceGroundingFlatId {
        SourceGroundingFlatId(self.0.to_flat_id(ranges))
    }
    pub fn from(grounding: Vec<IntCst>) -> Self {
        Self(Grounding(grounding))
    }
    pub fn inner(&self) -> &[IntCst] {
        &self.0.0
    }
}
impl Index<usize> for SourceGrounding {
    type Output = IntCst;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
