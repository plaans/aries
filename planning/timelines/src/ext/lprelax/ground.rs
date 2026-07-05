use aries_solver::prelude::*;

use crate::ext::{Grounding, GroundingFlatId};
use std::ops::Index;

// #[derive(Debug, Clone)]
// pub struct TransitionGroundingVecId(Vec<usize>);
//
// impl From<Vec<usize>> for TransitionGroundingVecId {
//     fn from(value: Vec<usize>) -> Self {
//         Self(value)
//     }
// }
// impl AsRef<[usize]> for TransitionGroundingVecId {
//     fn as_ref(&self) -> &[usize] {
//         &self.0
//     }
// }
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct TransitionGroundingFlatId(pub GroundingFlatId);
#[derive(Debug, Clone)]
pub struct TransitionGrounding(Grounding);

impl TransitionGrounding {
    // pub fn to_vec_id(&self, startvals: &[IntCst]) -> TransitionGroundingVecId {
    //     self.0.to_vec_id(startvals)
    // }
    // pub fn to_flat_id(&self, startvals: &[IntCst], dims: &[usize]) -> TransitionGroundingFlatId {
    //     self.0.to_flat_id(startvals, dims)
    // }
    pub fn to_flat_id(&self, ranges: &[(IntCst, IntCst)]) -> TransitionGroundingFlatId {
        TransitionGroundingFlatId(self.0.to_flat_id(ranges))
    }
    // pub fn from_vec_id(vec_id: &TransitionGroundingVecId, startvals: &[IntCst]) -> Self {
    //     Self(Grounding::from_vec_id(vec_id, startvals))
    // }
    // pub fn from_flat_id(flat_id: &TransitionGroundingFlatId, dims: &[usize], startvals: &[IntCst]) -> Self {
    //     Self(Grounding::from_flat_id(flat_id, dims, startvals))
    // }
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
