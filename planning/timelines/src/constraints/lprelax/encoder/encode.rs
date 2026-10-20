use aries_solver::core::views::Dom;
use itertools::Itertools;

use crate::SchedEncoder;
use crate::lprelax::transitions::TransitionType;

use super::LpRelaxEncoder;
use super::problem::{ColTag, LpRelaxProblem, RowExpr, RowExprType};

/// Pushes the "lifted rows": (lifted) presences of sources and transitions, as well as (lifted) supports between transitions.
pub(super) fn encode_problem_lifted(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, problem: &mut LpRelaxProblem) {
    let supports_sorted = encoder.supports_sorted.as_ref().unwrap();

    // [Lifted] A source is present if(f) its transitions are (and the empty source is present).
    {
        problem.push_row(RowExpr::new(
            RowExprType::Eq,
            vec![(1, ColTag::PresenceSource(None, None))],
            1,
            1,
        ));

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
                let terms = vec![
                    (1, ColTag::PresenceTransition(trans_id, None)),
                    (1, ColTag::PresenceSource(source, None)),
                ];
                problem.push_row(if presences_equivalent {
                    RowExpr::new_eq_single_lhs(terms)
                } else {
                    RowExpr::new_leq_single_lhs(terms)
                });
            }
        }
    }

    // [Lifted] A support implies the presence of its out-transition.
    // Only needed for supports into pure conditions, and only when conditions are disallowed from being supporters / out-transitions.
    // Indeed, these constraints are otherwise implied by the out-support constraints (when conditions are included in their sums).
    if !encoder.supports.with_condition_out_transitions {
        for &((out_trans_id, in_trans_id), _) in encoder.supports.unsorted_out() {
            if encoder.transitions.get(in_trans_id).tpe() == TransitionType::Cond {
                problem.push_row(RowExpr::new_leq_single_lhs(vec![
                    (1, ColTag::Support(out_trans_id, in_trans_id, None)),
                    (1, ColTag::PresenceTransition(out_trans_id, None)),
                ]));
            }
        }
    }

    // [Lifted] In-supports: the presence of a transition equals the total support it receives
    // (for the case of pure effects, including those of the initial state / empty source, see [`ColTag::SupportFromInitialState`]).
    {
        let with_initial_state_supports = crate::lprelax::ARIES_LPRELAX_WITH_INITIAL_STATE_SUPPORTS.get();

        let chunkby = supports_sorted
            .sorted_in()
            .iter()
            .chunk_by(|(in_trans_id, _)| in_trans_id);

        for (&in_trans_id, out_trans_ids) in &chunkby {
            let is_pure_effect = encoder.transitions.get(in_trans_id).tpe() == TransitionType::Eff;
            let support_from_initial_state = (with_initial_state_supports && is_pure_effect)
                .then_some((1, ColTag::SupportFromInitialState(None, in_trans_id)));

            let terms = std::iter::once((1, ColTag::PresenceTransition(in_trans_id, None)))
                .chain(out_trans_ids.map(|&(_, out_trans_id)| (1, ColTag::Support(out_trans_id, in_trans_id, None))))
                .chain(support_from_initial_state)
                .collect::<Vec<_>>();
            debug_assert!(terms.len() >= 2);

            // Without supports from the initial state, a pure effect need not be supported (it may be the first on its timeline).
            problem.push_row(if !with_initial_state_supports && is_pure_effect {
                RowExpr::new_geq_single_lhs(terms)
            } else {
                RowExpr::new_eq_single_lhs(terms)
            });
        }
    }

    // [Lifted] Out-supports: the presence of a transition is at least the total support it hands over
    // (leaving out supports into pure conditions when conditions are disallowed from being supporters / out-transitions).
    {
        let chunkby = supports_sorted
            .sorted_out()
            .iter()
            .chunk_by(|&((out_trans_id, _), _)| out_trans_id);

        for (&out_trans_id, supports) in &chunkby {
            let terms = std::iter::once((1, ColTag::PresenceTransition(out_trans_id, None)))
                .chain(
                    supports
                        .filter(|&&((_, in_trans_id), _)| {
                            encoder.supports.with_condition_out_transitions
                                || encoder.transitions.get(in_trans_id).tpe() != TransitionType::Cond
                        })
                        .map(|&((_, in_trans_id), _)| (1, ColTag::Support(out_trans_id, in_trans_id, None))),
                )
                .collect::<Vec<_>>();

            if terms.len() >= 2 {
                problem.push_row(RowExpr::new_geq_single_lhs(terms));
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
        let terms = std::iter::once((1, ColTag::PresenceSource(source, None)))
            .chain(
                source_groundings
                    .iter()
                    .map(|&g| (1, ColTag::PresenceSource(source, Some(g)))),
            )
            .collect::<Vec<_>>();
        problem.push_row(RowExpr::new_eq_single_lhs(terms));
    }

    // [Lifted-Ground] A transition is present iff one of its groundings is.
    {
        let chunkby = ground
            .iter_distinct_transition_groundings()
            .chunk_by(|&(trans_id, _)| trans_id);

        for (trans_id, groundings) in &chunkby {
            let terms = std::iter::once((1, ColTag::PresenceTransition(trans_id, None)))
                .chain(groundings.map(|(_, g)| (1, ColTag::PresenceTransition(trans_id, Some(g)))))
                .collect::<Vec<_>>();
            problem.push_row(RowExpr::new_eq_single_lhs(terms));
        }
    }

    // [Lifted-Ground] A ground transition is present if(f) one of the groundings of its source it comes from is.
    for (source, entries) in ground.iter_transition_groundings_by_source() {
        let chunkby = entries.chunk_by(|&&(trans_id, trans_grounding_id, _)| (trans_id, trans_grounding_id));

        for ((trans_id, trans_grounding_id), source_groundings) in &chunkby {
            let terms = std::iter::once((1, ColTag::PresenceTransition(trans_id, Some(trans_grounding_id))))
                .chain(source_groundings.map(|&(_, _, g)| (1, ColTag::PresenceSource(source, Some(g)))))
                .collect::<Vec<_>>();

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
                RowExpr::new_eq_single_lhs(terms)
            } else {
                RowExpr::new_leq_single_lhs(terms)
            });
        }
    }

    // [Ground] A support implies the presence of its out-transition (see the corresponding lifted rows).
    if !encoder.supports.with_condition_out_transitions {
        for (out_trans_id, support_grounding_id, groundings, in_trans_ids) in ground.iter_support_out_groups() {
            let presences = groundings
                .map(|g| (1, ColTag::PresenceTransition(out_trans_id, Some(g))))
                .collect::<Vec<_>>();

            for in_trans_id in in_trans_ids {
                if encoder.transitions.get(in_trans_id).tpe() == TransitionType::Cond {
                    let support = (
                        1,
                        ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id)),
                    );
                    let terms = std::iter::once(support).chain(presences.iter().copied()).collect();
                    problem.push_row(RowExpr::new_leq_single_lhs(terms));
                }
            }
        }
    }

    // [Lifted-Ground] A support is active iff one of its ground supports is.
    {
        let chunkby = ground
            .iter_support_groundings()
            .chunk_by(|&&(out_trans_id, in_trans_id, _)| (out_trans_id, in_trans_id));

        for ((out_trans_id, in_trans_id), entries) in &chunkby {
            let terms = std::iter::once((1, ColTag::Support(out_trans_id, in_trans_id, None)))
                .chain(entries.filter_map(|&(_, _, support_grounding_id)| {
                    support_grounding_id.map(|g| (1, ColTag::Support(out_trans_id, in_trans_id, Some(g))))
                }))
                .collect::<Vec<_>>();
            problem.push_row(RowExpr::new_eq_single_lhs(terms));
        }
    }

    // [Ground] In-supports: the presence of a group of groundings of a transition equals the total ground support it receives
    // (for a pure effect, including that of the initial state / empty source, see [`ColTag::SupportFromInitialState`]).
    {
        for (in_trans_id, (state_var_grounding_id, val_assignment), groundings, supports) in
            ground.iter_support_in_groups()
        {
            let is_pure_effect = val_assignment.is_none();

            let presences = groundings.map(|g| (1, ColTag::PresenceTransition(in_trans_id, Some(g))));
            let supports = supports.map(|(out_trans_id, support_grounding_id)| {
                (
                    1,
                    ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id)),
                )
            });

            // An effect may be supported by the initial state.
            let support_from_initial_state = (with_initial_state_supports && is_pure_effect).then_some((
                1,
                ColTag::SupportFromInitialState(Some(state_var_grounding_id), in_trans_id),
            ));

            let separator = presences.len();
            let terms = presences.chain(supports).chain(support_from_initial_state).collect();

            // Without supports from the initial state, a pure effect need not be supported (it may be the first on its timeline).
            let tpe = if !with_initial_state_supports && is_pure_effect {
                RowExprType::Geq
            } else {
                RowExprType::Eq
            };
            problem.push_row(RowExpr::new(tpe, terms, separator, 0));
        }
    }

    // [Ground] The initial state of a ground state variable supports at most one effect.
    // (When a single pure effect is concerned, implied by the column being defined on [0,1])
    if with_initial_state_supports {
        for (state_var_grounding_id, pure_effects) in ground.iter_effects_by_state_var() {
            if pure_effects.len() <= 1 {
                continue;
            }
            let terms = pure_effects
                .iter()
                .map(|&trans_id| {
                    (
                        1,
                        ColTag::SupportFromInitialState(Some(state_var_grounding_id), trans_id),
                    )
                })
                .chain([(1, ColTag::PresenceSource(None, None))])
                .collect();
            problem.push_row(RowExpr::new(RowExprType::Leq, terms, pure_effects.len(), 0));
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
            let terms = std::iter::once((1, ColTag::SupportFromInitialState(None, trans_id)))
                .chain(
                    ground
                        .iter_ground_state_vars_of(trans_id)
                        .map(|state_var_grounding_id| {
                            (
                                1,
                                ColTag::SupportFromInitialState(Some(state_var_grounding_id), trans_id),
                            )
                        }),
                )
                .collect::<Vec<_>>();
            problem.push_row(RowExpr::new_eq_single_lhs(terms));
        }
    }

    // [Ground] Out-supports: the presence of a group of groundings of a transition is at least the total ground support it hands over
    // (leaving out supports into pure conditions when conditions are disallowed from being supporters / out-transitions).
    for (out_trans_id, support_grounding_id, groundings, in_trans_ids) in ground.iter_support_out_groups() {
        let supports = in_trans_ids
            .filter(|&in_trans_id| {
                encoder.supports.with_condition_out_transitions
                    || encoder.transitions.get(in_trans_id).tpe() != TransitionType::Cond
            })
            .map(|in_trans_id| {
                (
                    1,
                    ColTag::Support(out_trans_id, in_trans_id, Some(support_grounding_id)),
                )
            })
            .collect::<Vec<_>>();
        if supports.is_empty() {
            continue;
        }

        let presences = groundings.map(|g| (1, ColTag::PresenceTransition(out_trans_id, Some(g))));
        let separator = presences.len();
        let terms = presences.chain(supports).collect();
        problem.push_row(RowExpr::new(RowExprType::Geq, terms, separator, 0));
    }

    // [Ground] At most one of a term's groundings can be active
    // (implied for the terms appearing in a single source).
    {
        let chunkby = ground.iter_distinct_term_assignments().chunk_by(|&(term, _)| term);

        for (term, values) in &chunkby {
            let terms = values
                .map(|(_, value)| (1, ColTag::TermGround(term, value)))
                .collect::<Vec<_>>();
            debug_assert!(!terms.is_empty());
            problem.push_row(RowExpr::new_leq_1(terms));
        }
    }

    // [Ground] A grounding of a term is active iff a ground source using it is active
    // (also needed for the terms appearing in a single source, for the binding of term groundings to the main model).
    {
        let chunkby = ground
            .iter_term_assignments_with_sources()
            .chunk_by(|&&(term, value, source, _)| (term, value, source));

        for ((term, value, source), entries) in &chunkby {
            let terms = std::iter::once((1, ColTag::TermGround(term, value)))
                .chain(entries.map(|&(_, _, _, g)| (1, ColTag::PresenceSource(source, Some(g)))))
                .collect::<Vec<_>>();

            debug_assert!(
                ctx.store
                    .state
                    .implies(encoder.get_source_prez(source, ctx), ctx.store.presence(term))
                    && ctx
                        .store
                        .state
                        .implies(ctx.store.presence(term), encoder.get_source_prez(source, ctx))
            );
            problem.push_row(RowExpr::new_eq_single_lhs(terms));
        }
    }
}
