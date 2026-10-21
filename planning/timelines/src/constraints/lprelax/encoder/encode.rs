use aries_linexpr::IntoLinExpr;
use aries_solver::core::views::Dom;
use itertools::Itertools;

use crate::SchedEncoder;
use crate::lprelax::transitions::TransitionType;

use super::LpRelaxEncoder;
use super::problem::{ColTag, LpRelaxProblem, sum};

/// Pushes the "lifted rows": (lifted) presences of sources and transitions, as well as (lifted) supports between transitions.
pub(super) fn encode_problem_lifted(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, problem: &mut LpRelaxProblem) {
    let supports_sorted = encoder.supports_sorted.as_ref().unwrap();

    // [Lifted] A source is present if(f) its transitions are (and the empty source is present).
    {
        problem.push_row(ColTag::PresenceSource(None, None).into_linexpr().eq(1));

        for (source, trans_ids) in encoder.iter_sources() {
            for &trans_id in trans_ids {
                // It is possible that sometimes the presence literal of the transition differs from that of the source
                // (in particular, as a result of complying with pddl set semantics, if the transition presence literal was replaced with a more specific one than the source's).
                // However, the transition's presence literal must always imply the source's.

                debug_assert!(ctx.store.state.implies(
                    encoder.transitions.get_prez(trans_id, ctx),
                    encoder.get_source_prez(source, ctx)
                ));
                let presences_equivalent = ctx.store.state.implies(
                    encoder.get_source_prez(source, ctx),
                    encoder.transitions.get_prez(trans_id, ctx),
                );

                // When the transition's and source's presence literals are truly equivalent, we can enforce equality. Otherwise, we can only enforce one side.
                let presence = ColTag::PresenceTransition(trans_id, None).into_linexpr();
                let source_presence = ColTag::PresenceSource(source, None);
                problem.push_row(if presences_equivalent {
                    presence.eq(source_presence)
                } else {
                    presence.leq(source_presence)
                });
            }
        }
    }

    // [Lifted] A support implies the presence of its out-transition.
    // Only needed for supports into conds, and only when conditions are disallowed from being supporters / out-transitions.
    // Indeed, these constraints are otherwise implied by the out-support constraints (when conditions are included in their sums).
    if !encoder.supports.with_condition_out_transitions {
        for &((out_trans_id, in_trans_id), _) in encoder.supports.unsorted_out() {
            if encoder.transitions.get(in_trans_id).tpe() == TransitionType::Cond {
                let support = ColTag::Support(out_trans_id, in_trans_id, None);
                let presence = ColTag::PresenceTransition(out_trans_id, None);
                problem.push_row(support.into_linexpr().leq(presence));
            }
        }
    }

    // [Lifted] In-supports: the presence of a transition equals the total support it receives
    // (for the case of effs, including those of the initial state / empty source, see [`ColTag::SupportFromInitialState`]).
    {
        let with_initial_state_supports = crate::lprelax::ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS.get();

        let chunkby = supports_sorted
            .sorted_in()
            .iter()
            .chunk_by(|(in_trans_id, _)| in_trans_id);

        for (&in_trans_id, out_trans_ids) in &chunkby {
            let is_eff = encoder.transitions.get(in_trans_id).tpe() == TransitionType::Eff;
            let support_from_initial_state =
                (with_initial_state_supports && is_eff).then_some(ColTag::SupportFromInitialState(None, in_trans_id));

            let presence = ColTag::PresenceTransition(in_trans_id, None).into_linexpr();
            let supports = out_trans_ids.map(|&(_, out_trans_id)| ColTag::Support(out_trans_id, in_trans_id, None));
            let supports = sum(supports.chain(support_from_initial_state));
            debug_assert!(!supports.terms().is_empty());

            // Without supports from the initial state, an eff need not be supported (it may be the first on its timeline).
            problem.push_row(if !with_initial_state_supports && is_eff {
                presence.geq(supports)
            } else {
                presence.eq(supports)
            });
        }
    }

    // [Lifted] Out-supports: the presence of a transition is at least the total support it hands over
    // (leaving out supports into conds when conditions are disallowed from being supporters / out-transitions).
    {
        let chunkby = supports_sorted
            .sorted_out()
            .iter()
            .chunk_by(|&((out_trans_id, _), _)| out_trans_id);

        for (&out_trans_id, entries) in &chunkby {
            let supports = entries
                .filter(|&&((_, in_trans_id), _)| {
                    encoder.supports.with_condition_out_transitions
                        || encoder.transitions.get(in_trans_id).tpe() != TransitionType::Cond
                })
                .map(|&((_, in_trans_id), _)| ColTag::Support(out_trans_id, in_trans_id, None));
            let supports = sum(supports);

            if !supports.terms().is_empty() {
                let presence = ColTag::PresenceTransition(out_trans_id, None);
                problem.push_row(presence.into_linexpr().geq(supports));
            }
        }
    }
}

/// Pushes the "ground rows": (ground) presences of sources and transitions, as well as (ground) supports between transitions,
/// and the rows decomposing the lifted columns into ground ones.
pub(super) fn encode_problem_ground(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, problem: &mut LpRelaxProblem) {
    let supports_sorted = encoder.supports_sorted.as_ref().unwrap();
    let ground = &encoder.ground;
    let with_initial_state_supports = crate::lprelax::ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS.get();

    // [Lifted-Ground] A source is present iff one of its groundings is.
    for (source, _) in encoder.iter_sources() {
        let source_groundings = ground.source_groundings(source);
        if source_groundings.is_empty() {
            // ? WARNING ? related to incomplete / partial groundings. [TODO]
            continue;
        }
        let groundings = source_groundings
            .iter()
            .map(|&g| ColTag::PresenceSource(source, Some(g)));
        problem.push_row(ColTag::PresenceSource(source, None).into_linexpr().eq(sum(groundings)));
    }

    // [Lifted-Ground] A transition is present iff one of its groundings is.
    for (trans_id, groundings) in ground.iter_transition_groundings() {
        let groundings = sum(groundings.map(|g| ColTag::PresenceTransition(trans_id, Some(g))));
        problem.push_row(ColTag::PresenceTransition(trans_id, None).into_linexpr().eq(groundings));
    }

    // [Lifted-Ground] A ground transition is present if(f) one of the groundings of its source it comes from is.
    for (trans_id, trans_grounding_id, source, source_groundings) in ground.iter_transition_groundings_with_sources() {
        let presence = ColTag::PresenceTransition(trans_id, Some(trans_grounding_id)).into_linexpr();
        let source_presences = sum(source_groundings.map(|g| ColTag::PresenceSource(source, Some(g))));

        // Same as for the lifted rows: the transition's presence literal must always imply the source's,
        // and we can enforce equality only when they are equivalent.
        debug_assert!(ctx.store.state.implies(
            encoder.transitions.get_prez(trans_id, ctx),
            encoder.get_source_prez(source, ctx)
        ));
        let presences_equivalent = ctx.store.state.implies(
            encoder.get_source_prez(source, ctx),
            encoder.transitions.get_prez(trans_id, ctx),
        );
        problem.push_row(if presences_equivalent {
            presence.eq(source_presences)
        } else {
            presence.leq(source_presences)
        });
    }

    // [Ground] A support implies the presence of its out-transition (see the corresponding lifted rows).
    if !encoder.supports.with_condition_out_transitions {
        for (out_trans_id, support_grounding_id, groundings, in_trans_ids) in ground.iter_support_out_groups() {
            let presences = sum(groundings.map(|g| ColTag::PresenceTransition(out_trans_id, Some(g))));

            for in_trans_id in in_trans_ids {
                if encoder.transitions.get(in_trans_id).tpe() == TransitionType::Cond {
                    let support = ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id));
                    problem.push_row(support.into_linexpr().leq(presences.clone()));
                }
            }
        }
    }

    // [Lifted-Ground] A support is active iff one of its ground supports is.
    for (out_trans_id, in_trans_id, support_groundings) in ground.iter_support_groundings() {
        let ground_supports = support_groundings.map(|g| ColTag::Support(out_trans_id, in_trans_id, Some(g)));
        let support = ColTag::Support(out_trans_id, in_trans_id, None);
        problem.push_row(support.into_linexpr().eq(sum(ground_supports)));
    }

    // [Ground] In-supports: the presence of a group of groundings of a transition equals the total ground support it receives
    // (for an eff, including that of the initial state / empty source, see [`ColTag::SupportFromInitialState`]).
    {
        for (in_trans_id, (state_var_grounding_id, val_assignment), groundings, supports) in
            ground.iter_support_in_groups()
        {
            let is_eff = val_assignment.is_none();

            let presences = sum(groundings.map(|g| ColTag::PresenceTransition(in_trans_id, Some(g))));
            let supports = supports.map(|(out_trans_id, support_grounding_id)| {
                ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id))
            });

            // An effect may be supported by the initial state.
            let support_from_initial_state = (with_initial_state_supports && is_eff).then_some(
                ColTag::SupportFromInitialState(Some(state_var_grounding_id), in_trans_id),
            );
            let supports = sum(supports.chain(support_from_initial_state));

            // Without supports from the initial state, an eff need not be supported (it may be the first on its timeline).
            problem.push_row(if !with_initial_state_supports && is_eff {
                presences.geq(supports)
            } else {
                presences.eq(supports)
            });
        }
    }

    // [Ground] The initial state of a ground state variable supports at most one effect.
    // (When a single eff is concerned, implied by the column being defined on [0,1])
    if with_initial_state_supports {
        for (state_var_grounding_id, effs) in ground.iter_effects_by_state_var() {
            if effs.len() <= 1 {
                continue;
            }
            let supports = effs
                .iter()
                .map(|&trans_id| ColTag::SupportFromInitialState(Some(state_var_grounding_id), trans_id));
            problem.push_row(ColTag::PresenceSource(None, None).into_linexpr().geq(sum(supports)));
        }
    }

    // [Lifted-Ground] A support from the initial state is the total of its ground ones
    // (for the effects whose lifted in-support includes it).
    if with_initial_state_supports {
        let lifted_effects = supports_sorted
            .sorted_in()
            .iter()
            .map(|&(in_trans_id, _)| in_trans_id)
            .dedup()
            .filter(|&trans_id| encoder.transitions.get(trans_id).tpe() == TransitionType::Eff);

        for trans_id in lifted_effects {
            let ground_supports = ground
                .iter_ground_state_vars_of(trans_id)
                .map(|state_var_grounding_id| ColTag::SupportFromInitialState(Some(state_var_grounding_id), trans_id));
            let support = ColTag::SupportFromInitialState(None, trans_id);
            problem.push_row(support.into_linexpr().eq(sum(ground_supports)));
        }
    }

    // [Ground] Out-supports: the presence of a group of groundings of a transition is at least the total ground support it hands over
    // (leaving out supports into conds when conditions are disallowed from being supporters / out-transitions).
    for (out_trans_id, support_grounding_id, groundings, in_trans_ids) in ground.iter_support_out_groups() {
        let supports = in_trans_ids
            .filter(|&in_trans_id| {
                encoder.supports.with_condition_out_transitions
                    || encoder.transitions.get(in_trans_id).tpe() != TransitionType::Cond
            })
            .map(|in_trans_id| ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id)));
        let supports = sum(supports);
        if supports.terms().is_empty() {
            continue;
        }

        let presences = sum(groundings.map(|g| ColTag::PresenceTransition(out_trans_id, Some(g))));
        problem.push_row(presences.geq(supports));
    }

    // [Ground] At most one of a term's groundings can be active
    // (implied for the terms appearing in a single source).
    for (term, values) in ground.iter_term_values() {
        let groundings = sum(values.map(|value| ColTag::TermGround(term, value)));
        debug_assert!(!groundings.terms().is_empty());
        problem.push_row(groundings.leq(1));
    }

    // [Ground] A grounding of a term is active iff a ground source using it is active
    // (also needed for the terms appearing in a single source, for the binding of term groundings to the main model).
    for (term, value, source, source_groundings) in ground.iter_term_assignments_with_sources() {
        let source_presences = sum(source_groundings.map(|g| ColTag::PresenceSource(source, Some(g))));

        debug_assert!(
            ctx.store
                .state
                .implies(encoder.get_source_prez(source, ctx), ctx.store.presence(term))
                && ctx
                    .store
                    .state
                    .implies(ctx.store.presence(term), encoder.get_source_prez(source, ctx))
        );
        problem.push_row(ColTag::TermGround(term, value).into_linexpr().eq(source_presences));
    }
}
