use aries_solver::lang::Lit;

use super::{TransitionId, TransitionType, Transitions};
use crate::{IntTerm, SchedEncoder};

/// Whether two sequences of terms may be equal, i.e. differ at no position where both terms are constants.
fn may_be_equal(terms: impl IntoIterator<Item = IntTerm>, other_terms: impl IntoIterator<Item = IntTerm>) -> bool {
    terms
        .into_iter()
        .zip(other_terms)
        .all(|(term, other)| !(term.is_cst() && other.is_cst() && term.constant != other.constant))
}

/// Stores (generalized) potential lifted supports (or support links) between transitions.
///
/// This generalized notion of support builds up on effect-condition causal link / causal support and extends it to all types of transitions.
///
/// A support `(out, in)` encodes the possibility for out-transition (or supporter transition) `out` to
/// hand over the value of its state variable to the in-transition (or supportee/supported transition ) `in`,
/// i.e. for `out` to be transition right before `in` on their (common, ground) state variable.
///
/// The support is said to be "active" when this actually holds.
/// And when we say that "`out` supports `in`" we usually mean that the support link between them is active.
#[derive(Clone, Default)]
pub(crate) struct Supports {
    /// Whether condition-transitions are allowed to play the role of out-transition / supporter transition.
    pub with_condition_out_transitions: bool,
    unsorted_out: Vec<((TransitionId, TransitionId), Option<Lit>)>,
}

impl Supports {
    pub fn from(transitions: &Transitions, ctx: &SchedEncoder, with_condition_out_transitions: bool) -> Self {
        // Supports due to the causal links in the main encoding.
        let supports_causal_links = ctx.causal_links.get_links().filter_map(|cl| {
            let out_trans_id = transitions.of_effect(cl.eff_id)?;
            let in_trans_id = transitions.of_condition(cl.cond_id)?;

            debug_assert_eq!(
                transitions.get_state_var(out_trans_id, ctx).fluent,
                transitions.get_state_var(in_trans_id, ctx).fluent,
            );
            (!transitions.necessarily_precedes(in_trans_id, out_trans_id, ctx))
                .then_some(((out_trans_id, in_trans_id), Some(cl.active)))
        });

        let effects = || transitions.iter_of_effects().map(|(_, trans_id)| trans_id);
        let conditions = || transitions.iter_of_conditions().map(|(_, trans_id)| trans_id);

        // Whether a transition may support another (as far as their constant terms and time bounds tell)
        let may_support = move |out_trans_id: TransitionId, in_trans_id: TransitionId| {
            let out_state_var = transitions.get_state_var(out_trans_id, ctx);
            let in_state_var = transitions.get_state_var(in_trans_id, ctx);
            out_trans_id != in_trans_id
                && out_state_var.fluent == in_state_var.fluent
                && may_be_equal(out_state_var.args.iter().copied(), in_state_var.args.iter().copied())
                && !transitions.necessarily_precedes(in_trans_id, out_trans_id, ctx)
        };
        let is_cond_eff = move |trans_id: TransitionId| transitions.get(trans_id).tpe() == TransitionType::CondEff;

        // Supports from effects to other effects
        let supports_from_effects_to_others = effects().flat_map(move |out_trans_id| {
            effects()
                .filter(move |&in_trans_id| !is_cond_eff(in_trans_id) && may_support(out_trans_id, in_trans_id))
                .map(move |in_trans_id| ((out_trans_id, in_trans_id), None::<Lit>))
        });

        // Supports from conditions to other conditions and to effects (if desired, see `with_condition_out_transitions` flag)
        let supports_from_conditions = with_condition_out_transitions
            .then(|| {
                conditions()
                    // out-transition must not be a cond-eff (these are already included in the original causal link supports)
                    .filter(move |&out_trans_id| !is_cond_eff(out_trans_id))
                    .flat_map(move |out_trans_id| {
                        let to_effects = effects().filter(move |&in_trans_id| {
                            !is_cond_eff(in_trans_id) && may_support(out_trans_id, in_trans_id)
                        });
                        let to_conditions = conditions().filter(move |&in_trans_id| {
                            may_support(out_trans_id, in_trans_id)
                                && may_be_equal(
                                    transitions.get_view(out_trans_id, ctx).pre_val,
                                    transitions.get_view(in_trans_id, ctx).pre_val,
                                )
                        });
                        to_effects
                            .chain(to_conditions)
                            .map(move |in_trans_id| ((out_trans_id, in_trans_id), None::<Lit>))
                    })
            })
            .into_iter()
            .flatten();

        let unsorted_out = (supports_causal_links
            .chain(supports_from_effects_to_others)
            .chain(supports_from_conditions))
        .inspect(|&((out_trans_id, in_trans_id), _)| {
            debug_assert!(out_trans_id != in_trans_id);
            debug_assert!(!transitions.necessarily_precedes(in_trans_id, out_trans_id, ctx));
            debug_assert!(
                transitions.get(out_trans_id).tpe() != TransitionType::CondEff
                    || transitions.get_source(out_trans_id, ctx).is_some()
            );
        })
        .collect();

        Self {
            unsorted_out,
            with_condition_out_transitions,
        }
    }

    /// Iterator over potential supports, with a corresponding activation literal if there is one / if it is known.
    ///
    /// (Unchunkable linearly because unsorted)
    pub fn unsorted_out(&self) -> &[((TransitionId, TransitionId), Option<Lit>)] {
        &self.unsorted_out
    }

    pub fn sort(&self) -> SupportsSorted {
        let mut sorted_out = Vec::with_capacity(self.unsorted_out.len());
        let mut sorted_in = Vec::with_capacity(self.unsorted_out.len());

        for &((out_trans_id, in_trans_id), active) in &self.unsorted_out {
            sorted_out.push(((out_trans_id, in_trans_id), active));
            sorted_in.push((in_trans_id, out_trans_id));
        }
        sorted_out.sort_unstable();
        sorted_in.sort_unstable();

        SupportsSorted { sorted_out, sorted_in }
    }
}

/// Offers sorted views of (potential) supports between transitions (see [`Supports`]), allowing direct chunking.
#[derive(Clone, Default)]
pub(crate) struct SupportsSorted {
    sorted_out: Vec<((TransitionId, TransitionId), Option<Lit>)>,
    sorted_in: Vec<(TransitionId, TransitionId)>,
}

impl SupportsSorted {
    /// Sorted (and thus directly chunkable) iterator over potential supports
    /// (with a corresponding activation literal if there is one / if it is known)
    /// following the (`out`,`in`) shape (first transition is the out-transition, second one is in-transition).
    pub fn sorted_out(&self) -> &[((TransitionId, TransitionId), Option<Lit>)] {
        &self.sorted_out
    }
    /// Sorted (and thus directly chunkable) iterator over potential supports
    /// following the (`in`,`out`) shape (first transition is the in-transition, second one is out-transition).
    pub fn sorted_in(&self) -> &[(TransitionId, TransitionId)] {
        &self.sorted_in
    }
}

#[cfg(test)]
mod tests {
    use aries_solver::prelude::Lit;
    use itertools::Itertools;

    use crate::lprelax::tests::visitall::{VisitAllLine, build_and_encode_visitall_line};

    use crate::encoder::{CausalLink, SchedEncoder};
    use crate::lprelax::transitions::{Transition, TransitionId, TransitionType, Transitions, supports::Supports};

    #[test]
    fn test_supports() {
        let encoder = build_and_encode_visitall_line(
            &VisitAllLine {
                num_locs: 5,
                num_moves: 4,
            },
            false,
        );

        let transitions = Transitions::new_unambiguous(&encoder);

        let supports_without_conditions_out_transitions = Supports::from(&transitions, &encoder, false);
        for &((out_trans_id, in_trans_id), active) in supports_without_conditions_out_transitions.unsorted_out() {
            test_supports_aux(&transitions, &encoder, out_trans_id, in_trans_id, active);
        }

        let supports_with_conditions_out_transitions = Supports::from(&transitions, &encoder, true);
        for &((out_trans_id, in_trans_id), active) in supports_with_conditions_out_transitions.unsorted_out() {
            test_supports_aux(&transitions, &encoder, out_trans_id, in_trans_id, active);
        }
    }

    fn test_supports_aux(
        transitions: &Transitions,
        encoder: &SchedEncoder,
        out_trans_id: TransitionId,
        in_trans_id: TransitionId,
        active: Option<Lit>,
    ) {
        if let Some(active) = active {
            assert!(transitions.get(in_trans_id).tpe() != TransitionType::Eff);

            assert!({
                let (eff_id, cond_id) = match (transitions.get(out_trans_id), transitions.get(in_trans_id)) {
                    (
                        Transition::Eff(eff_id) | Transition::CondEff(_, eff_id),
                        Transition::Cond(cond_id) | Transition::CondEff(cond_id, _),
                    ) => (eff_id, cond_id),
                    _ => unreachable!(),
                };
                encoder.causal_links.get_links().contains(&CausalLink {
                    eff_id,
                    cond_id,
                    active,
                })
            });
        } else {
            assert!(!matches!(
                (transitions.get(out_trans_id), transitions.get(in_trans_id),),
                (
                    Transition::CondEff(_, _) | Transition::Eff(_),
                    Transition::Cond(_) | Transition::CondEff(_, _),
                )
            ));
        }
    }
}
