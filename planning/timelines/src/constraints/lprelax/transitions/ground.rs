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
            EffectOp::Step(_) => todo!("step effects (relaxed as nonsimple for now)"),
            // EffectOp::Erase => None, // TODO ?
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
            EffectOp::Step(_) => todo!("step effects (relaxed as nonsimple for now)"),
        };
        let pre_val = if view.pre_val.is_some() { values.pop() } else { None };

        TransitionTermsEvaluation {
            state_var_args: values,
            pre_val,
            post_op,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use aries_solver::core::IntCst;

    use crate::SchedEncoder;
    use crate::analysis::Source;
    use crate::analysis::grounding::ParametersAssignment;
    use crate::lprelax::tests::visitall::{VisitAllLine, build_and_encode_visitall_line};
    use crate::lprelax::transitions::{TransitionType, Transitions};

    /// A transition's type and fluent, with its evaluated state variable args, pre-value and post-value.
    type Evaluation = (TransitionType, String, Vec<IntCst>, Option<IntCst>, Option<IntCst>);

    fn evaluation(
        tpe: TransitionType,
        fluent: &str,
        args: &[IntCst],
        pre_val: Option<IntCst>,
        post_op: Option<IntCst>,
    ) -> Evaluation {
        (tpe, fluent.to_string(), args.to_vec(), pre_val, post_op)
    }

    /// The evaluations of the transitions of `source`, against the grounding giving its parameters the values `values`.
    fn evaluations(
        transitions: &Transitions,
        ctx: &SchedEncoder,
        source: Source,
        values: &[IntCst],
    ) -> BTreeSet<Evaluation> {
        let source_grounding = ParametersAssignment::from(values.to_vec());
        transitions
            .of_source(source)
            .iter()
            .map(|&trans_id| {
                let eval = transitions.evaluate_terms(trans_id, &source_grounding, ctx);
                evaluation(
                    transitions.get(trans_id).tpe(),
                    &transitions.get_state_var(trans_id, ctx).fluent,
                    eval.state_var_args_eval(),
                    eval.pre_val_eval(),
                    eval.post_op_eval(),
                )
            })
            .collect()
    }

    /// The parameters of `move(?curpos, ?nextpos)` are replaced by their values in the grounding.
    #[test]
    fn evaluate_terms_replaces_parameters_by_their_values() {
        let ctx = build_and_encode_visitall_line(
            &VisitAllLine {
                num_locs: 3,
                num_moves: 1,
            },
            false,
        );
        let transitions = Transitions::new_unambiguous(&ctx);
        let (move_source, _) = transitions
            .iter_of_sources()
            .find(|(source, _)| source.is_some())
            .unwrap();

        // ?curpos = 100 and ?nextpos = 200
        assert_eq!(
            evaluations(&transitions, &ctx, move_source, &[100, 200]),
            BTreeSet::from([
                // (at-robot ?curpos): from true to false
                evaluation(TransitionType::CondEff, "at-robot", &[100], Some(1), Some(0)),
                // (connected ?curpos ?nextpos)
                evaluation(TransitionType::Cond, "connected", &[100, 200], Some(1), Some(1)),
                // (at-robot ?nextpos) and (visited ?nextpos)
                evaluation(TransitionType::Eff, "at-robot", &[200], None, Some(1)),
                evaluation(TransitionType::Eff, "visited", &[200], None, Some(1)),
            ])
        );
    }

    /// Without parameters to replace (the empty source), the evaluation is made of the transitions' constant terms.
    #[test]
    fn evaluate_terms_keeps_constants() {
        let ctx = build_and_encode_visitall_line(
            &VisitAllLine {
                num_locs: 2,
                num_moves: 1,
            },
            false,
        );
        let transitions = Transitions::new_unambiguous(&ctx);
        let loc = |name: &str| ctx.sched.objects.object_id(name).unwrap();
        let (l0, l1) = (loc("loc-x0"), loc("loc-x1"));

        assert_eq!(
            evaluations(&transitions, &ctx, None, &[]),
            BTreeSet::from([
                // the goals
                evaluation(TransitionType::Cond, "visited", &[l0], Some(1), Some(1)),
                evaluation(TransitionType::Cond, "visited", &[l1], Some(1), Some(1)),
                // the initial state
                evaluation(TransitionType::Eff, "at-robot", &[l0], None, Some(1)),
                evaluation(TransitionType::Eff, "visited", &[l0], None, Some(1)),
                evaluation(TransitionType::Eff, "connected", &[l0, l1], None, Some(1)),
                evaluation(TransitionType::Eff, "connected", &[l1, l0], None, Some(1)),
            ])
        );
    }
}
