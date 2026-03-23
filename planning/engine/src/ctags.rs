use std::sync::Arc;

use derivative::Derivative;
use planx::ActionRef;

use crate::repair::potential_effects::PotentialEffects;

/// Tag for a cosntraint imposed in the scheduling model
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum CTag {
    /// Constraint enforcing the i-th goal
    EnforceGoal(usize),
    /// Cosntraint enforcing the given condition of the i-th operator (action in the plan)
    Support { operator_id: usize, cond: ActionCondition },
    /// Constraint that disables a potential effect of an action template
    DisablePotentialEffect(PotentialEffect),
    /// Constraint that forces the usage of an actual effect of the action template
    EnforceEffect(ActionEffect),
}

impl CTag {
    pub fn to_repair(&self) -> Option<Repair> {
        match self {
            CTag::EnforceGoal(_) => None,
            CTag::Support { cond, .. } => Some(Repair::RmCond(cond.clone())),
            CTag::DisablePotentialEffect(potential_effect) => Some(Repair::AddEff(potential_effect.clone())),
            CTag::EnforceEffect(action_effect) => Some(Repair::RmEff(action_effect.clone())),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ActionCondition {
    pub action: ActionRef,
    pub condition_id: usize,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ActionEffect {
    pub action: ActionRef,
    /// Index of the effect in the action's model
    pub effect_id: usize,
}

#[derive(Derivative)]
#[derivative(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct PotentialEffect {
    pub action_id: ActionRef,
    /// Index of the effect in the list of potential effects
    pub effect_id: usize,
    #[derivative(Debug = "ignore", PartialEq = "ignore", PartialOrd = "ignore", Ord = "ignore")]
    pub all_effects: Arc<PotentialEffects>,
}

impl PotentialEffect {
    pub fn get(&self) -> &crate::repair::potential_effects::PotentialEffect {
        &self.all_effects.for_action(&self.action_id)[self.effect_id]
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum Repair {
    RmCond(ActionCondition),
    AddEff(PotentialEffect),
    RmEff(ActionEffect),
}
