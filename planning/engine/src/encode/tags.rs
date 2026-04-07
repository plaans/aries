use std::collections::BTreeSet;

use planx::{ActionRef, Message, Model, errors::Spanned};

use crate::plans::lifted_plan::LiftedPlan;

/// Tag for a cosntraint imposed in the scheduling model
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum Tag {
    /// Constraint enforcing the i-th goal
    EnforceGoal(usize),
    /// Constraint enforcing the given condition of the i-th operator (action in the plan)
    Support { operator_id: usize, cond: ActionCondition },
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ActionCondition {
    /// Name of the action in which the condition appears
    pub action: ActionRef,
    /// Index of the condition in the action.
    pub condition_id: usize,
}

/// Extends a base bessage to display all culprits in it.
pub fn format_culprit_set(mut msg: Message, culprits: &BTreeSet<Tag>, model: &Model, plan: &LiftedPlan) -> Message {
    for culprit in culprits {
        match culprit {
            Tag::EnforceGoal(g) => {
                let g = &model.goals[*g];
                let g = model.env.node(g);
                let annot = g.error("Unsatisfied goal");
                msg = msg.snippet(annot);
            }
            Tag::Support { operator_id, cond } => {
                let operator = &plan.operations[*operator_id];
                let annot = operator.error(format!("non applicable (operator #{operator_id})"));
                msg = msg.snippet(annot);
                // for all previous operators in the plan, display them if they have span (indicating they were read from a file)
                for prev in &plan.operations[..*operator_id] {
                    if let Some(prev_span) = prev.span.as_ref() {
                        msg = msg.show(prev_span)
                    }
                }
                let action = model.actions.get_action(&cond.action).unwrap();
                let cond_expr = action.conditions[cond.condition_id].cond;
                let annot = model
                    .env
                    .node(cond_expr)
                    .info(format!("unsatisfiable condition for operator {operator_id}"));
                msg = msg.snippet(annot).show(cond.action.span.as_ref().unwrap());
            }
        }
    }
    msg
}
