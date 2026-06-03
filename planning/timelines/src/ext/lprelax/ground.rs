use aries::prelude::*;

use crate::ext::Grounding;
use std::ops::Index;

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct TransitionGroundingFlatId(pub usize);

impl From<usize> for TransitionGroundingFlatId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
impl From<TransitionGroundingFlatId> for usize {
    fn from(value: TransitionGroundingFlatId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
pub struct TransitionGrounding(Grounding);

impl TransitionGrounding {
    pub fn to_flat_id(&self, ranges: &[(IntCst, IntCst)]) -> TransitionGroundingFlatId {
        self.0.to_flat_id(ranges)
    }
    pub fn from(grounding: Vec<IntCst>) -> Self {
        Self(Grounding(grounding))
    }
    pub fn inner(&self) -> &[IntCst] {
        &self.0.0
    }
}
impl Index<usize> for TransitionGrounding {
    type Output = IntCst;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
