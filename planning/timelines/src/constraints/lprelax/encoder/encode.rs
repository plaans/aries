use aries_solver::core::views::Dom;
use itertools::Itertools;

use crate::SchedEncoder;
use crate::analysis::transitions::TransitionType;
use crate::constraints::lprelax::LpRelaxEncoder;
use crate::constraints::lprelax::encoder::problem::{ColTag, LpRelaxProblem, RowExpr, RowExprType};

pub fn encode_problem_lifted(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, problem: &mut LpRelaxProblem) {
    // [Lifted] A source is present if(f) its transitions are
    // TODO: optimize iterations ? (flattened ?)
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
                // However, the latter must always imply the former.

                debug_assert!(ctx.store.state.implies(
                    encoder.transitions.get_prez(trans_id, ctx),
                    encoder.get_source_prez(source, ctx)
                ));
                let presences_equivalent = ctx.store.state.implies(
                    encoder.get_source_prez(source, ctx),
                    encoder.transitions.get_prez(trans_id, ctx),
                );

                // When the transition's and source's presence literals are truly equivalent, we can enforce equality. Otherwise, we can only enforce one side.
                let expr = if presences_equivalent {
                    RowExpr::new_eq_single_lhs(vec![
                        (1, ColTag::PresenceTransition(trans_id, None)),
                        (1, ColTag::PresenceSource(source, None)),
                    ])
                } else {
                    RowExpr::new_leq_single_lhs(vec![
                        (1, ColTag::PresenceTransition(trans_id, None)),
                        (1, ColTag::PresenceSource(source, None)),
                    ])
                };

                problem.push_row(expr);
            }
        }
    }

    // [Lifted] Support between two transitions implies presence of both of them
    // NOTE: There's no need to enforce theses constraints for all cases, as the inflow and outflow constraints are stronger (see below).
    //       They're only actually needed for out-conditions when the in-transition is a "pure-condition", as this case is not implied by outflow constraints.
    {
        for &((out_trans_id, in_trans_id), _) in encoder.supports.unsorted_out() {
            if !encoder.supports.with_condition_out_transitions
                && encoder.transitions.get(in_trans_id).tpe() == TransitionType::Cond
            {
                problem.push_row(RowExpr::new_leq_single_lhs(vec![
                    (1, ColTag::Support(out_trans_id, in_trans_id, None)),
                    (1, ColTag::PresenceTransition(out_trans_id, None)),
                ]));
            }
        }
    }

    // [Lifted] Forbid two transitions from mutually supporting each other ("trivial cycles")
    {
        let mut seen = vec![];

        for &((out_trans_id, in_trans_id), _) in encoder.supports_sorted.as_ref().unwrap().sorted_out() {
            seen.push((out_trans_id, in_trans_id));

            if seen.binary_search(&(in_trans_id, out_trans_id)).is_ok() {
                problem.push_row(RowExpr::new_leq_1(vec![
                    (1, ColTag::Support(out_trans_id, in_trans_id, None)),
                    (1, ColTag::Support(in_trans_id, out_trans_id, None)),
                ]));
            }
        }
        debug_assert!(seen.is_sorted());
    }

    // [Lifted] "Inflow" constraints: [TODO]
    {
        let chunkby = encoder
            .supports_sorted
            .as_ref()
            .unwrap()
            .sorted_in()
            .iter()
            .chunk_by(|(in_trans_id, _)| in_trans_id);

        for (&in_trans_id, out_trans_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![(1, ColTag::PresenceTransition(in_trans_id, None))];
                res.append(
                    &mut out_trans_ids
                        .into_iter()
                        .map(|&(_, out_trans_id)| (1, ColTag::Support(out_trans_id, in_trans_id, None)))
                        .collect(),
                );
                res
            };
            debug_assert!(terms.len() >= 2);

            let expr = if encoder.transitions.are_recovered_closed_world_default_effects_empty()
                && encoder.transitions.get(in_trans_id).tpe() == TransitionType::Eff
            {
                RowExpr::new_geq_single_lhs(terms)
            } else {
                RowExpr::new_eq_single_lhs(terms)
            };

            problem.push_row(expr);
        }
    }

    // [Lifted] "Outflow" constraints: [TODO]
    {
        let chunkby = encoder
            .supports_sorted
            .as_ref()
            .unwrap()
            .sorted_out()
            .iter()
            .chunk_by(|&((out_trans_id, _), _)| out_trans_id);

        for (&out_trans_id, in_trans_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![(1, ColTag::PresenceTransition(out_trans_id, None))];
                res.append(
                    &mut in_trans_ids
                        .into_iter()
                        .filter(|&&((_, in_trans_id), _)| {
                            encoder.supports.with_condition_out_transitions
                                || encoder.transitions.get(in_trans_id).tpe() != TransitionType::Cond
                        })
                        .map(|&((_, in_trans_id), _)| (1, ColTag::Support(out_trans_id, in_trans_id, None)))
                        .collect(),
                );
                res
            };

            if terms.len() >= 2 {
                let expr = RowExpr::new_geq_single_lhs(terms);
                problem.push_row(expr);
            }
        }
    }
}

pub fn encode_problem_ground(encoder: &LpRelaxEncoder, ctx: &SchedEncoder, problem: &mut LpRelaxProblem) {
    // [Lifted-Ground] Source ground decomposition (a source is present iff one its groundings is)
    {
        for (source, _) in encoder.iter_sources() {
            let terms = {
                let mut res = vec![(1, ColTag::PresenceSource(source, None))];
                res.append(
                    &mut encoder
                        .sources_ground
                        .get(source)
                        .iter()
                        .map(|&source_grounding_id| (1, ColTag::PresenceSource(source, Some(source_grounding_id))))
                        .collect(),
                );
                res
            };
            if terms.len() >= 2 {
                let expr = RowExpr::new_eq_single_lhs(terms);
                problem.push_row(expr);
            } else {
                // ? WARNING ? related to incomplete / partial groundings. [TODO]
                continue;
            }
        }
    }

    // [Lifted-Ground] Transition ground decomposition
    {
        let chunkby = encoder
            .transitions_ground
            .iter_all_unsourced()
            .chunk_by(|&(trans_id, _)| trans_id);

        for (trans_id, trans_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![(1, ColTag::PresenceTransition(trans_id, None))];
                res.append(
                    &mut trans_groundings_ids
                        .into_iter()
                        .map(|(_, trans_grounding_id)| {
                            (1, ColTag::PresenceTransition(trans_id, Some(trans_grounding_id)))
                        })
                        .collect(),
                );
                res
            };
            debug_assert!(terms.len() >= 2);

            let expr = RowExpr::new_eq_single_lhs(terms);
            problem.push_row(expr);
        }
    }

    // [Lifted-Ground] Ground transition is only active if(f) a compatible grounding of its source is active
    {
        for (source, iter) in encoder.transitions_ground.iter_all_sourced() {
            let chunkby = iter.chunk_by(|(trans_id, trans_grounding_id, _)| (trans_id, trans_grounding_id));

            for ((&trans_id, &trans_grounding_id), source_groundings_ids) in chunkby.into_iter() {
                let terms = {
                    let mut res = vec![(1, ColTag::PresenceTransition(trans_id, Some(trans_grounding_id)))];
                    res.append(
                        &mut source_groundings_ids
                            .into_iter()
                            .map(|&(_, _, source_grounding_id)| {
                                (1, ColTag::PresenceSource(source, Some(source_grounding_id)))
                            })
                            .collect(),
                    );
                    res
                };
                debug_assert!(terms.len() >= 2);

                // Same as for the lifted case:
                // It is possible that sometimes the presence literal of the transition differs from that of the source
                // (in particular, as a result of complying with pddl set semantics, if the transition presence literal was replaced with a more specific one than the source's).
                // However, the latter must always imply the former.

                debug_assert!(ctx.store.state.implies(
                    encoder.transitions.get_prez(trans_id, ctx),
                    encoder.get_source_prez(source, ctx)
                ));
                let presences_equivalent = ctx.store.state.implies(
                    encoder.get_source_prez(source, ctx),
                    encoder.transitions.get_prez(trans_id, ctx),
                );

                // When the transition's and source's presence literals are truly equivalent, we can enforce equality. Otherwise, we can only enforce one side.
                let expr = if presences_equivalent {
                    RowExpr::new_eq_single_lhs(terms)
                } else {
                    RowExpr::new_leq_single_lhs(terms)
                };

                problem.push_row(expr);
            }
        }
    }

    // [Ground] Support between two (ground) transitions implies presence of both of them
    // NOTE: There's no need to enforce theses constraints for all cases, as the (ground) inflow and outflow constraints are stronger (see below).
    //       They're only actually needed for out-conditions when the in-transition is a "pure-condition", as this case is not implied by (ground) outflow constraints.
    {
        for &(out_trans_id, in_trans_id, trans_groundings_ids) in encoder.supports_ground.iter_all() {
            let Some((out_trans_grounding_id, in_trans_grounding_id)) = trans_groundings_ids else {
                continue;
            };

            if !encoder.supports.with_condition_out_transitions
                && encoder.transitions.get(in_trans_id).tpe() == TransitionType::Cond
            {
                problem.push_row(RowExpr::new_leq_single_lhs(vec![
                    (
                        1,
                        ColTag::Support(
                            out_trans_id,
                            in_trans_id,
                            Some((out_trans_grounding_id, in_trans_grounding_id)),
                        ),
                    ),
                    (
                        1,
                        ColTag::PresenceTransition(out_trans_id, Some(out_trans_grounding_id)),
                    ),
                ]));
            }
        }
    }

    // [Lifted-Ground] Supports ground decomposition
    {
        let chunkby = encoder
            .supports_ground
            .iter_all()
            .chunk_by(|(out_trans_id, in_trans_id, _)| (out_trans_id, in_trans_id));

        for ((&out_trans_id, &in_trans_id), trans_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![(1, ColTag::Support(out_trans_id, in_trans_id, None))];
                res.append(
                    &mut trans_groundings_ids
                        .into_iter()
                        .filter_map(|&(_, _, trans_groundings_ids)| {
                            trans_groundings_ids.map(|(out_trans_grounding_id, in_trans_grounding_id)| {
                                (
                                    1,
                                    ColTag::Support(
                                        out_trans_id,
                                        in_trans_id,
                                        Some((out_trans_grounding_id, in_trans_grounding_id)),
                                    ),
                                )
                            })
                        })
                        .collect(),
                );
                res
            };

            let expr = RowExpr::new_eq_single_lhs(terms);
            problem.push_row(expr);
        }
    }

    // [Ground] "Inflow" constraints: [TODO]
    {
        let chunkby = encoder
            .supports_ground
            .iter_in_all()
            .chunk_by(|(in_trans_id, in_trans_grounding_id, _)| (in_trans_id, in_trans_grounding_id));

        for ((&in_trans_id, &in_trans_grounding_id), out_trans_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![(1, ColTag::PresenceTransition(in_trans_id, Some(in_trans_grounding_id)))];
                res.append(
                    &mut out_trans_groundings_ids
                        .into_iter()
                        .filter_map(|&(_, _, x)| {
                            x.map(|(out_trans_id, out_trans_grounding_id)| {
                                (
                                    1,
                                    ColTag::Support(
                                        out_trans_id,
                                        in_trans_id,
                                        Some((out_trans_grounding_id, in_trans_grounding_id)),
                                    ),
                                )
                            })
                        })
                        .collect(),
                );
                res
            };

            let expr = if encoder.transitions.are_recovered_closed_world_default_effects_empty()
                && encoder.transitions.get(in_trans_id).tpe() == TransitionType::Eff
            {
                // In the case where we do not recover and use "missing" initial effects,
                // the inflow constraints for (all) effects are slightly weaker.
                RowExpr::new_geq_single_lhs(terms)
            } else {
                RowExpr::new_eq_single_lhs(terms)
            };

            problem.push_row(expr);
        }

        if encoder.transitions.are_recovered_closed_world_default_effects_empty() {
            // In the case where we do not recover and use "missing" initial effects,
            // the inflow constraints for (all) effects are slightly weaker (see above).
            // This is (partially? FIXME[proof?]) compensated by the following constraints,
            // which state that the *sum* of inflows into (ground effects) with the *same state variable* (so, independent of their value) is upper bounded by 1.

            let chunkby = encoder
                .supports_ground
                .iter_in_all()
                .filter(|(in_trans_id, _, _)| encoder.transitions.get(*in_trans_id).tpe() == TransitionType::Eff)
                .map(|(in_trans_id, in_trans_grounding_id, out_trans_groundings)| {
                    (in_trans_grounding_id, in_trans_id, out_trans_groundings)
                })
                .sorted_unstable_by_key(|&(in_trans_grounding_id, _, _)| *in_trans_grounding_id)
                .chunk_by(|&(in_trans_grounding_id, _, _)| in_trans_grounding_id.state_var_grounding_id);

            for (_, x) in chunkby.into_iter() {
                let terms = x
                    .into_iter()
                    .filter_map(|(&in_trans_grounding_id, &in_trans_id, out_trans_grounding)| {
                        out_trans_grounding.map(|(out_trans_id, out_trans_grounding_id)| {
                            (
                                1,
                                ColTag::Support(
                                    out_trans_id,
                                    in_trans_id,
                                    Some((out_trans_grounding_id, in_trans_grounding_id)),
                                ),
                            )
                        })
                    })
                    .collect::<Vec<_>>();

                if !terms.is_empty() {
                    let expr = RowExpr::new_leq_1(terms);
                    problem.push_row(expr);
                }
            }
        }
    }

    // [Ground] "Outflow" constraints: [TODO]
    {
        let chunkby = encoder
            .supports_ground
            .iter_out_all()
            .chunk_by(|(out_trans_id, out_trans_grounding_id, _)| (out_trans_id, out_trans_grounding_id));

        for ((&out_trans_id, &out_trans_grounding_id), in_trans_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![(
                    1,
                    ColTag::PresenceTransition(out_trans_id, Some(out_trans_grounding_id)),
                )];
                res.append(
                    &mut in_trans_groundings_ids
                        .into_iter()
                        .filter(|&&(_, _, (in_trans_id, _))| {
                            encoder.supports.with_condition_out_transitions
                                || encoder.transitions.get(in_trans_id).tpe() != TransitionType::Cond
                        })
                        .map(|&(_, _, (in_trans_id, in_trans_grounding_id))| {
                            (
                                1,
                                ColTag::Support(
                                    out_trans_id,
                                    in_trans_id,
                                    Some((out_trans_grounding_id, in_trans_grounding_id)),
                                ),
                            )
                        })
                        .collect(),
                );
                res
            };

            if terms.len() >= 2 {
                let expr = RowExpr::new_geq_single_lhs(terms);
                problem.push_row(expr);
            }
        }
    }

    // // [Ground] Forbid one (ground) transitions from mutually supporting each other
    // //          TODO: ? is this actually needed ? -> (this may not necessarily be useful / enough for cases of eff-eff supports, as any value could be usef (or eff-condeff))
    // if super::ARIES_LPRELAX_GROUND_2CYCLES.get() {
    //     let ground_supports_iter_sorted_fully = groundings.supports().iter_all().sorted();
    //     let mut seen = vec![];
    //
    //     for &(out_trans_id, in_trans_id, trans_groundings_ids) in ground_supports_iter_sorted_fully {
    //         let Some((out_trans_grounding_id, in_trans_grounding_id)) = trans_groundings_ids else {
    //             continue;
    //         };
    //
    //         seen.push((
    //             out_trans_id,
    //             in_trans_id,
    //             out_trans_grounding_id,
    //             in_trans_grounding_id,
    //         ));
    //
    //         if seen
    //             .binary_search(&(
    //                 in_trans_id,
    //                 out_trans_id,
    //                 in_trans_grounding_id,
    //                 out_trans_grounding_id,
    //             ))
    //             .is_ok()
    //         {
    //             let expr = RowExpr::Leq1(vec![
    //                 ColTag::SupportGround(
    //                     out_trans_id,
    //                     in_trans_id,
    //                     out_trans_grounding_id,
    //                     in_trans_grounding_id,
    //                 ),
    //                 ColTag::SupportGround(
    //                     in_trans_id,
    //                     out_trans_id,
    //                     in_trans_grounding_id,
    //                     out_trans_grounding_id,
    //                 ),
    //             ]);
    //             problem.push_row(expr);
    //         }
    //     }
    //     debug_assert!(seen.is_sorted());
    // }

    // [Ground] At most one of a term's groundings can be active
    {
        let chunkby = encoder
            .terms_ground
            .iter_sorted_all_only_assignments()
            .chunk_by(|&(term, _)| term);

        for (term, values) in chunkby.into_iter() {
            let terms = values
                .into_iter()
                .map(|(_, value)| (1, ColTag::TermGround(term, value)))
                .collect::<Vec<_>>();

            debug_assert!(!terms.is_empty());

            let expr = RowExpr::new_leq_1(terms);
            problem.push_row(expr);
        }
    }

    // [Ground] A grounding of a term is active iff a ground transition using it is active
    // [Ground] ---------------------------------------------- source --------------------
    {
        let chunkby = encoder
            .terms_ground
            .iter_sorted_all_for_transitions()
            .chunk_by(|&(term, value, trans_id, _)| (term, value, trans_id));

        for ((&term, &value, &trans_id), trans_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![(1, ColTag::TermGround(term, value))];
                res.append(
                    &mut trans_groundings_ids
                        .into_iter()
                        .map(|&(_, _, _, trans_grounding_id)| {
                            (1, ColTag::PresenceTransition(trans_id, Some(trans_grounding_id)))
                        })
                        .collect(),
                );
                res
            };
            debug_assert!(terms.len() >= 2);

            debug_assert!(
                ctx.store
                    .state
                    .implies(encoder.transitions.get_prez(trans_id, ctx), ctx.store.presence(term))
            );
            let presences_equivalent = ctx
                .store
                .state
                .implies(ctx.store.presence(term), encoder.transitions.get_prez(trans_id, ctx));

            let expr = if presences_equivalent {
                RowExpr::new_eq_single_lhs(terms)
            } else {
                RowExpr::new_geq_single_lhs(terms)
            };
            problem.push_row(expr);
        }

        let chunkby = encoder
            .terms_ground
            .iter_sorted_all_for_sources()
            .chunk_by(|&(term, value, source, _)| (term, value, source));

        for ((&term, &value, &source), sources_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![(1, ColTag::TermGround(term, value))];
                res.append(
                    &mut sources_groundings_ids
                        .into_iter()
                        .map(|&(_, _, _, source_grounding_id)| {
                            (1, ColTag::PresenceSource(source, Some(source_grounding_id)))
                        })
                        .collect(),
                );
                res
            };
            debug_assert!(terms.len() >= 2);

            debug_assert!(
                ctx.store
                    .state
                    .implies(encoder.get_source_prez(source, ctx), ctx.store.presence(term))
                    && ctx
                        .store
                        .state
                        .implies(ctx.store.presence(term), encoder.get_source_prez(source, ctx))
            );

            let expr = RowExpr::new_eq_single_lhs(terms);
            problem.push_row(expr);
        }
    }
}
