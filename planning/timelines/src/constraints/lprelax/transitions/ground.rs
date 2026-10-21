use aries_solver::core::IntCst;

use super::{TransitionId, Transitions};

use crate::analysis::grounding::ParametersAssignment;
use crate::{EffectOp, IntTerm, SchedEncoder};

/// Encapsulates the evaluation of a transition's terms (assignment to constant).
/// Effectively corresponds to the transition's grounding.
///
/// Mirrors the structure of [`super::TransitionView`].
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransitionTermsEvaluation {
    state_var_args: smallvec::SmallVec<[IntCst; 4]>,
    pre_val: Option<IntCst>,
    post_op: EffectOp,
}
impl TransitionTermsEvaluation {
    /// See [`super::TransitionView::state_var`]
    pub fn state_var_args_eval(&self) -> &[IntCst] {
        &self.state_var_args
    }
    /// See [`super::TransitionView::pre_val`]
    pub fn pre_val_eval(&self) -> Option<IntCst> {
        self.pre_val
    }
    /// See [`super::TransitionView::post_op`]
    pub fn post_op_eval(&self) -> Option<IntCst> {
        match &self.post_op {
            EffectOp::Assign(term) => {
                debug_assert!(term.is_cst());
                Some(term.constant)
            }
            EffectOp::Step(term) => {
                debug_assert!(term.is_cst());
                Some(term.constant)
            } // EffectOp::Erase => None, // TODO ?
        }
    }
}

impl Transitions {
    /// Evaluates each term of a transition (see [`super::TransitionView::terms`]) against a grounding of its source.
    fn iter_evaluated_terms(
        &self,
        trans_id: TransitionId,
        source_grounding: &ParametersAssignment,
        ctx: &SchedEncoder,
    ) -> impl Iterator<Item = (IntTerm, IntCst)> {
        self.get_view(trans_id, ctx)
            .terms()
            .zip(&self.transition_terms_indices_in_source[trans_id])
            .map(|(term, index)| (term, index.map_or(term.constant, |i| source_grounding[i])))
    }

    /// Evaluates the terms of a transition (effectively returning its grounding).
    pub fn evaluate_terms(
        &self,
        trans_id: TransitionId,
        source_grounding: &ParametersAssignment,
        ctx: &SchedEncoder,
    ) -> TransitionTermsEvaluation {
        let view = self.get_view(trans_id, ctx);
        let mut values: smallvec::SmallVec<[IntCst; 4]> = self
            .iter_evaluated_terms(trans_id, source_grounding, ctx)
            .map(|(_, value)| value)
            .collect();

        // The terms end with the pre-value (if any) and the post-operation's term.
        let post_op_value = IntTerm::int_cst(values.pop().unwrap());
        let post_op = match view.post_op {
            EffectOp::Assign(_) => EffectOp::Assign(post_op_value),
            EffectOp::Step(_) => EffectOp::Step(post_op_value),
        };
        let pre_val = if view.pre_val.is_some() { values.pop() } else { None };

        TransitionTermsEvaluation {
            state_var_args: values,
            pre_val,
            post_op,
        }
    }
}
