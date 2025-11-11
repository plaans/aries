use aries_planning_model::ActionRef;

/// Tag for a cosntraint imposed in the scheduling model
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CTag {
    /// Constraint enforcing the i-th goal
    EnforceGoal(usize),
    /// Cosntraint enforcing the given condition of the i-th operator (action in the plan)
    Support { operator_id: usize, cond: ActionCondition },
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActionCondition {
    pub action: ActionRef,
    pub condition_id: usize,
}
