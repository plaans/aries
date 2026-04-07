use crate::{ConstraintID, EffectId};

#[derive(Debug, PartialEq, Eq)]
pub enum TransitionId {
    /// Value is the index of a condition (constraint) in a reference vector of constraints.
    Cond(ConstraintID), 
    /// Value is the index/identified of an effect in a collection of them.
    Eff(EffectId), 
    /// Combination of Cond and Eff variants.
    CondEff(ConstraintID, EffectId)
}

pub struct Transitions {
    lifted: Vec<TransitionId>,
    //ground: DirectIdMap<TransitionId, todo!()>, // TODO
}
impl Transitions {
    pub fn from_lifted(lifted: Vec<TransitionId>) -> Self {
        Self {
            lifted,
        }
    }
}