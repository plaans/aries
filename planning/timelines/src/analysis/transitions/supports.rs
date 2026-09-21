use aries_solver::lang::Lit;

use super::{TransitionId, TransitionType, Transitions};
use crate::SchedEncoder;

#[derive(Clone, Default)]
pub(crate) struct Supports {
    pub with_condition_out_transitions: bool,
    unsorted_out: Vec<((TransitionId, TransitionId), Option<Lit>)>,
}
impl Supports {
    pub fn from(transitions: &Transitions, ctx: &SchedEncoder, with_condition_out_transitions: bool) -> Self {
        // Supporting stemming from the causal links in the main encoding.

        let supports_causal_links = ctx.causal_links.get_links().filter_map(|cl| {
            // println!(
            //     "{:?} {:?}",
            //     (cl.eff_id, ctx.sched.effects.get(cl.eff_id)),
            //     (cl.cond_id, ctx.causal_links.conditions.get(cl.cond_id))
            // );
            let out_trans_id = transitions.of_effect(cl.eff_id)?;
            let in_trans_id = transitions.of_condition(cl.cond_id)?;

            debug_assert_eq!(
                transitions.get_state_var(out_trans_id, ctx).fluent,
                transitions.get_state_var(in_trans_id, ctx).fluent,
            );
            Some(((out_trans_id, in_trans_id), Some(cl.active)))
        });

        // Supports from effects (including "missing" ones) to other effects (non-initial) effects
        //
        // Supports from them to anything other than an effect are not needed (they otherwise wouldn't have been ignored in the main encoding).
        // As such: This (obviously) assumes that the main encoding soundly omits initial effects that may not support the conditions.

        let iter_effects_info = || {
            transitions
                .iter_of_effects()
                .map(|(_, trans_id)| transitions.get_effect_info(trans_id, ctx).unwrap())
        };
        let supports_from_effects_to_others = iter_effects_info().flat_map(move |(out_eff_id, _)| {
            let out_trans_id = transitions.of_effect(out_eff_id).unwrap();

            iter_effects_info().flat_map(move |(in_eff_id, _)| {
                let in_trans_id = transitions.of_effect(in_eff_id).unwrap();

                if out_trans_id == in_trans_id
                    || transitions.get(in_trans_id).tpe() == TransitionType::CondEff
                    || transitions.get_source(in_trans_id, ctx).is_none() // in-transition must not correspond to an initial effect
                    || transitions.get_state_var(out_trans_id, ctx).fluent
                        != transitions.get_state_var(in_trans_id, ctx).fluent
                {
                    return None;
                }
                debug_assert!(
                    transitions.get(in_trans_id).tpe() == TransitionType::Eff
                        && transitions.get_source(in_trans_id, ctx).is_some()
                );

                let out_terms = transitions.get_terms(out_trans_id, ctx);
                let in_terms = transitions.get_terms(in_trans_id, ctx);

                let incompatible_args = out_terms.args().iter().zip(in_terms.args()).any(|(out_term, in_term)| {
                    out_term.is_cst() && in_term.is_cst() && out_term.constant != in_term.constant
                });

                (!incompatible_args).then_some(((out_trans_id, in_trans_id), None::<Lit>))
            })
        });

        let iter_conditions = || {
            transitions
                .iter_of_conditions()
                .map(|(_, trans_id)| transitions.get_condition(trans_id, ctx).unwrap())
        };

        // If desired, see `with_pure_conditions` flag:
        // Supports from (non-goal) conditions to other conditions and to (non-initial effects).

        let supports_from_conditions = with_condition_out_transitions
            .then(|| {
                iter_conditions().flat_map(move |(out_cond_id, _)| {
                    let out_trans_id = transitions.of_condition(out_cond_id).unwrap();

                    // out-transition must not correspond to a goal (i.e. initial) condition
                    // and must not be a cond-eff (these are already included in the original causal link supports)
                    (transitions.get_source(out_trans_id, ctx).is_some()
                        && transitions.get(out_trans_id).tpe() != TransitionType::CondEff)
                        .then(|| {
                            let to_effects = iter_effects_info().flat_map(move |(in_eff_id, _)| {
                                let in_trans_id = transitions.of_effect(in_eff_id).unwrap();

                                if out_trans_id == in_trans_id
                                || transitions.get(in_trans_id).tpe() == TransitionType::CondEff
                                || transitions.get_source(in_trans_id, ctx).is_none() // in-transition must not correspond to an initial effect
                                || transitions.get_state_var(out_trans_id, ctx).fluent
                                    != transitions.get_state_var(in_trans_id, ctx).fluent
                                {
                                    return None;
                                }
                                debug_assert!(
                                    transitions.get(in_trans_id).tpe() == TransitionType::Eff
                                        && transitions.get_source(in_trans_id, ctx).is_some()
                                );

                                let out_terms = transitions.get_terms(out_trans_id, ctx);
                                let in_terms = transitions.get_terms(in_trans_id, ctx);

                                let incompatible_args =
                                    out_terms.args().iter().zip(in_terms.args()).any(|(out_term, in_term)| {
                                        out_term.is_cst() && in_term.is_cst() && out_term.constant != in_term.constant
                                    });

                                (!incompatible_args).then_some(((out_trans_id, in_trans_id), None::<Lit>))
                            });
                            let to_conditions = iter_conditions().flat_map(move |(in_cond_id, _)| {
                                let in_trans_id = transitions.of_condition(in_cond_id).unwrap();

                                if out_trans_id == in_trans_id
                                    || transitions.get_state_var(out_trans_id, ctx).fluent
                                        != transitions.get_state_var(in_trans_id, ctx).fluent
                                {
                                    return None;
                                }

                                let out_terms = transitions.get_terms(out_trans_id, ctx);
                                let in_terms = transitions.get_terms(in_trans_id, ctx);

                                let incompatible_args =
                                    out_terms.args().iter().zip(in_terms.args()).any(|(out_term, in_term)| {
                                        out_term.is_cst() && in_term.is_cst() && out_term.constant != in_term.constant
                                    });

                                let incompatible_vals = match (out_terms.val(), in_terms.val()) {
                                    (Some(out_val), Some(in_val)) => {
                                        out_val.is_cst() && in_val.is_cst() && out_val.constant != in_val.constant
                                    }
                                    _ => false,
                                };

                                (!incompatible_args && !incompatible_vals)
                                    .then_some(((out_trans_id, in_trans_id), None::<Lit>))
                            });

                            to_effects.chain(to_conditions)
                        })
                        .into_iter()
                        .flatten()
                })
            })
            .into_iter()
            .flatten();

        let unsorted_out = (supports_causal_links
            .chain(supports_from_effects_to_others)
            .chain(supports_from_conditions))
        .filter(|&((out_trans_id, in_trans_id), _)| out_trans_id != in_trans_id)
        .inspect(|&((out_trans_id, in_trans_id), _)| {
            debug_assert!(
                transitions.get(out_trans_id).tpe() != TransitionType::Cond
                    || transitions.get_source(in_trans_id, ctx).is_some()
            );
            debug_assert!(
                transitions.get(out_trans_id).tpe() != TransitionType::CondEff
                    || transitions.get_source(in_trans_id, ctx).is_some()
            );
            debug_assert!(
                transitions.get(in_trans_id).tpe() != TransitionType::Eff
                    || transitions.get_source(in_trans_id, ctx).is_some()
            );
        })
        .collect();

        Self {
            unsorted_out,
            with_condition_out_transitions,
        }
    }

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

#[derive(Clone, Default)]
pub(crate) struct SupportsSorted {
    sorted_out: Vec<((TransitionId, TransitionId), Option<Lit>)>,
    sorted_in: Vec<(TransitionId, TransitionId)>,
}

impl SupportsSorted {
    pub fn sorted_out(&self) -> &[((TransitionId, TransitionId), Option<Lit>)] {
        &self.sorted_out
    }
    pub fn sorted_in(&self) -> &[(TransitionId, TransitionId)] {
        &self.sorted_in
    }
}

#[cfg(test)]
mod tests {
    use aries_solver::prelude::Lit;
    use itertools::Itertools;

    use crate::{
        analysis::transitions::{
            Transition, TransitionId, TransitionType, Transitions,
            examples::visitall::{VisitAllLine, build_and_encode_visitall_line},
            supports::Supports,
        },
        encoder::{CausalLink, SchedEncoder},
    };

    #[test]
    fn test_supports() {
        let encoder = build_and_encode_visitall_line(
            &VisitAllLine {
                num_locs: 5,
                num_moves: 4,
            },
            false,
        );

        let transitions = Transitions::new_unambiguous(&encoder, true);

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

            assert!(
                transitions
                    .get_effect_info(in_trans_id, encoder)
                    .is_none_or(|(in_eff_id, _)| !transitions.is_recovered_closed_world_default(in_eff_id))
            );

            if transitions
                .get_effect_info(out_trans_id, encoder)
                .is_some_and(|(out_eff_id, _)| transitions.is_recovered_closed_world_default(out_eff_id))
            {
                assert!(transitions.get(out_trans_id).tpe() == TransitionType::Eff);
                assert!(transitions.get_source(out_trans_id, encoder).is_none());
            }
        }
    }
}
