use std::sync::Arc;

use aries_planning_model::ActionRef;

/// Tag for a cosntraint imposed in the scheduling model
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum CTag {
    /// Constraint enforcing the i-th goal
    EnforceGoal(usize),
    /// Cosntraint enforcing the given condition of the i-th operator (action in the plan)
    Support {
        operator_id: usize,
        cond: ActionCondition,
    },
    DisablePotentialEffect(PotentialEffect),
}

impl CTag {
    pub fn to_repair(&self) -> Option<Repair> {
        match self {
            CTag::EnforceGoal(_) => None,
            CTag::Support { cond, .. } => Some(Repair::RmCond(cond.clone())),
            CTag::DisablePotentialEffect(potential_effect) => Some(Repair::AddEff(potential_effect.clone())),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ActionCondition {
    pub action: ActionRef,
    pub condition_id: usize,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct PotentialEffect {
    pub action_id: ActionRef,
    /// Index of the effect in the list of potential effects
    pub effect_id: usize,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum Repair {
    RmCond(ActionCondition),
    AddEff(PotentialEffect),
}
