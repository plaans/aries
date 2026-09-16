mod groundings;
mod lifted;
mod problem;

use aries_solver::lang::Lit;
pub(crate) use problem::*;

use aries_solver::core::views::Dom;
use itertools::Itertools;

use crate::encoder::SchedEncoder;
use crate::ext::lprelax::LpRelaxEncoder;
use crate::ext::lprelax::encoding::groundings::LpRelaxEncodingGroundingsInfo;
use crate::ext::lprelax::encoding::lifted::LpRelaxEncodingLiftedSupportsSorted;
use crate::ext::lprelax::transitions::TransitionId;
use crate::ext::{Source, SourceGrounding};

#[derive(Clone)]
pub(crate) struct LpRelaxEncoding {
    encoder: LpRelaxEncoder,
    groundings: LpRelaxEncodingGroundingsInfo,
    lifted_supports_sorted: LpRelaxEncodingLiftedSupportsSorted,
    ready: bool,
}
impl LpRelaxEncoding {
    pub fn new(encoder: LpRelaxEncoder) -> Self {
        Self {
            encoder,
            ready: false,
            groundings: Default::default(),
            lifted_supports_sorted: Default::default(),
        }
    }

    // Interns a grounding of a source.
    // WARNING: adding duplicate groundings (for the same source) will result in a panic (in debug mode).
    pub fn post_ground_source(
        &mut self,
        source: Source,
        source_grounding: SourceGrounding,
        ctx: &SchedEncoder,
        doms: Option<&crate::Domains>,
    ) {
        self.ready = false;
        self.groundings
            .post_ground_source(source, source_grounding, &self.encoder, ctx, doms);
    }

    pub fn build(&mut self, ctx: &SchedEncoder, doms: Option<&crate::Domains>) -> LpRelaxProblem {
        let time = std::time::Instant::now();
        println!("|- Lifted part encoding construction started");

        let mut problem = LpRelaxProblem::default();

        self.lifted_supports_sorted = LpRelaxEncodingLiftedSupportsSorted::from(&self.encoder, ctx, doms);

        encode_problem_lifted(self, ctx, &self.lifted_supports_sorted, &mut problem);
        println!(
            "|- Lifted part encoding construction ended (run time: {})",
            time.elapsed().as_secs_f64()
        );

        let time = std::time::Instant::now();
        println!("|- Ground part encoding construction started");

        {
            self.groundings.transitions_sort();

            let time = std::time::Instant::now();
            println!("|--- Ground part encoding construction: support building started");

            self.groundings.supports_build(&self.lifted_supports_sorted);

            println!(
                "|--- Ground part encoding construction: support building ended (run time {})",
                time.elapsed().as_secs_f64()
            );

            self.groundings.terms_sort();
        }

        encode_problem_ground(self, ctx, &self.groundings, &mut problem);
        println!(
            "|- Ground part encoding construction ended (run time: {})",
            time.elapsed().as_secs_f64()
        );

        self.groundings.mark_ready();
        self.ready = true;

        problem
    }

    #[allow(dead_code)]
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn includes_recovered_mies(&self) -> bool {
        self.encoder.transitions.includes_recovered_mies()
    }

    pub fn iter_terms_assignments(&self) -> impl Iterator<Item = (crate::IntTerm, aries_solver::core::IntCst)> {
        self.groundings.terms().iter_sorted_all_only_assignments()
    }

    pub fn iter_sources(&self, ctx: &SchedEncoder) -> impl Iterator<Item = Source> {
        self.encoder.iter_sources(ctx)
    }
    pub fn iter_transitions_of_source(&self, source: Source) -> impl Iterator<Item = TransitionId> {
        self.encoder.transitions.of_source(source).iter().copied()
    }
    pub fn get_source_prez(&self, source: Source, ctx: &SchedEncoder) -> Lit {
        self.encoder.get_source_prez(source, ctx)
    }
    pub fn get_transition_prez(&self, transition_id: TransitionId, ctx: &SchedEncoder) -> Lit {
        self.encoder.transitions.get_prez(transition_id, ctx)
    }
    pub fn iter_supports(&self) -> impl Iterator<Item = ((TransitionId, TransitionId), Option<Lit>)> {
        self.lifted_supports_sorted.out().iter().copied()
    }
}

fn encode_problem_lifted(
    encoding: &LpRelaxEncoding,
    ctx: &SchedEncoder,
    lifted_supports_sorted: &LpRelaxEncodingLiftedSupportsSorted,
    problem: &mut LpRelaxProblem,
) {
    let is_transition_eff = |transition_id| encoding.encoder.transitions.is_pure_eff(transition_id);
    let is_transition_cond = |transition_id| encoding.encoder.transitions.is_pure_cond(transition_id);

    // [Lifted] A source is present if(f) its transitions are
    // TODO: optimize iterations ? (flattened ?)
    {
        problem.push_row(RowExpr::new(
            RowExprType::Eq,
            vec![ColTag::PresenceSource(None, None)],
            1,
            1,
        ));

        for source in encoding.iter_sources(ctx) {
            for transition_id in encoding.iter_transitions_of_source(source) {
                // It is possible that sometimes the presence literal of the transition differs from that of the source
                // (in particular, as a result of complying with pddl set semantics, if the transition presence literal was replaced with a more specific one than the source's).
                // However, the latter must always imply the former.

                debug_assert!(ctx.store.state.implies(
                    encoding.get_transition_prez(transition_id, ctx),
                    encoding.get_source_prez(source, ctx)
                ));
                let presences_equivalent = ctx.store.state.implies(
                    encoding.get_source_prez(source, ctx),
                    encoding.get_transition_prez(transition_id, ctx),
                );

                // When the transition's and source's presence literals are truly equivalent, we can enforce equality. Otherwise, we can only enforce one side.
                let expr = if presences_equivalent {
                    RowExpr::new_eq_single_lhs(vec![
                        ColTag::PresenceTransition(transition_id, None),
                        ColTag::PresenceSource(source, None),
                    ])
                } else {
                    RowExpr::new_leq_single_lhs(vec![
                        ColTag::PresenceTransition(transition_id, None),
                        ColTag::PresenceSource(source, None),
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
        for &((out_transition_id, in_transition_id), _) in lifted_supports_sorted.out() {
            if is_transition_cond(in_transition_id) {
                problem.push_row(RowExpr::new_leq_single_lhs(vec![
                    ColTag::Support(out_transition_id, in_transition_id, None),
                    ColTag::PresenceTransition(out_transition_id, None),
                ]));
            }
        }
    }

    // [Lifted] Forbid two transitions from mutually supporting each other ("trivial cycles")
    {
        let mut seen = vec![];

        for &((out_transition_id, in_transition_id), _) in lifted_supports_sorted.out() {
            seen.push((out_transition_id, in_transition_id));

            if seen.binary_search(&(in_transition_id, out_transition_id)).is_ok() {
                problem.push_row(RowExpr::new_leq_1(vec![
                    ColTag::Support(out_transition_id, in_transition_id, None),
                    ColTag::Support(in_transition_id, out_transition_id, None),
                ]));
            }
        }
        debug_assert!(seen.is_sorted());
    }

    // [Lifted] "Inflow" constraints: [TODO]
    {
        let chunkby = lifted_supports_sorted
            .in_()
            .iter()
            .chunk_by(|(in_transition_id, _)| in_transition_id);

        for (&in_transition_id, out_transitions_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::PresenceTransition(in_transition_id, None)];
                res.append(
                    &mut out_transitions_ids
                        .into_iter()
                        .map(|&(_, out_transition_id)| ColTag::Support(out_transition_id, in_transition_id, None))
                        .collect(),
                );
                res
            };
            debug_assert!(terms.len() >= 2);

            let expr = if !encoding.includes_recovered_mies() && is_transition_eff(in_transition_id) {
                RowExpr::new_geq_single_lhs(terms)
            } else {
                RowExpr::new_eq_single_lhs(terms)
            };

            problem.push_row(expr);
        }
    }

    // [Lifted] "Outflow" constraints: [TODO]
    {
        let chunkby = lifted_supports_sorted
            .out()
            .iter()
            .chunk_by(|&((out_transition_id, _), _)| out_transition_id);

        for (&out_transition_id, in_transitions_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::PresenceTransition(out_transition_id, None)];
                res.append(
                    &mut in_transitions_ids
                        .into_iter()
                        .filter(|&&((_, in_transition_id), _)| !is_transition_cond(in_transition_id))
                        .map(|&((_, in_transition_id), _)| ColTag::Support(out_transition_id, in_transition_id, None))
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

fn encode_problem_ground(
    encoding: &LpRelaxEncoding,
    ctx: &SchedEncoder,
    groundings: &LpRelaxEncodingGroundingsInfo,
    problem: &mut LpRelaxProblem,
) {
    let is_transition_eff = |transition_id| encoding.encoder.transitions.is_pure_eff(transition_id);
    let is_transition_cond = |transition_id| encoding.encoder.transitions.is_pure_cond(transition_id);

    // [Lifted-Ground] Source ground decomposition (a source is present iff one its groundings is)
    {
        for source in encoding.iter_sources(ctx) {
            let terms = {
                let mut res = vec![ColTag::PresenceSource(source, None)];
                res.append(
                    &mut groundings
                        .sources()
                        .get(source)
                        .iter()
                        .map(|&source_grounding_id| ColTag::PresenceSource(source, Some(source_grounding_id)))
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
        let chunkby = groundings
            .transitions()
            .iter_all_unsourced()
            .chunk_by(|&(transition_id, _)| transition_id);

        for (transition_id, transition_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::PresenceTransition(transition_id, None)];
                res.append(
                    &mut transition_groundings_ids
                        .into_iter()
                        .map(|(_, transition_grounding_id)| {
                            ColTag::PresenceTransition(transition_id, Some(transition_grounding_id))
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
        for (source, iter) in groundings.transitions().iter_all_sourced() {
            let chunkby =
                iter.chunk_by(|(transition_id, transition_grounding_id, _)| (transition_id, transition_grounding_id));

            for ((&transition_id, &transition_grounding_id), source_groundings_ids) in chunkby.into_iter() {
                let terms = {
                    let mut res = vec![ColTag::PresenceTransition(transition_id, Some(transition_grounding_id))];
                    res.append(
                        &mut source_groundings_ids
                            .into_iter()
                            .map(|&(_, _, source_grounding_id)| {
                                ColTag::PresenceSource(source, Some(source_grounding_id))
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
                    encoding.get_transition_prez(transition_id, ctx),
                    encoding.get_source_prez(source, ctx)
                ));
                let presences_equivalent = ctx.store.state.implies(
                    encoding.get_source_prez(source, ctx),
                    encoding.get_transition_prez(transition_id, ctx),
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
        for &(out_transition_id, in_transition_id, transition_groundings_ids) in groundings.supports().iter_all() {
            let Some((out_transition_grounding_id, in_transition_grounding_id)) = transition_groundings_ids else {
                continue;
            };

            if is_transition_cond(in_transition_id) {
                problem.push_row(RowExpr::new_leq_single_lhs(vec![
                    ColTag::Support(
                        out_transition_id,
                        in_transition_id,
                        Some((out_transition_grounding_id, in_transition_grounding_id)),
                    ),
                    ColTag::PresenceTransition(out_transition_id, Some(out_transition_grounding_id)),
                ]));
            }
        }
    }

    // [Lifted-Ground] Supports ground decomposition
    {
        let chunkby = groundings
            .supports()
            .iter_all()
            .chunk_by(|(out_transition_id, in_transition_id, _)| (out_transition_id, in_transition_id));

        for ((&out_transition_id, &in_transition_id), transitions_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::Support(out_transition_id, in_transition_id, None)];
                res.append(
                    &mut transitions_groundings_ids
                        .into_iter()
                        .filter_map(|&(_, _, transitions_groundings_ids)| {
                            transitions_groundings_ids.map(
                                |(out_transition_grounding_id, in_transition_grounding_id)| {
                                    ColTag::Support(
                                        out_transition_id,
                                        in_transition_id,
                                        Some((out_transition_grounding_id, in_transition_grounding_id)),
                                    )
                                },
                            )
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
        let chunkby =
            groundings
                .supports()
                .iter_in_all()
                .chunk_by(|(in_transition_id, in_transition_grounding_id, _)| {
                    (in_transition_id, in_transition_grounding_id)
                });

        for ((&in_transition_id, &in_transition_grounding_id), out_transitions_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::PresenceTransition(
                    in_transition_id,
                    Some(in_transition_grounding_id),
                )];
                res.append(
                    &mut out_transitions_groundings_ids
                        .into_iter()
                        .filter_map(|&(_, _, x)| {
                            x.map(|(out_transition_id, out_transition_grounding_id)| {
                                ColTag::Support(
                                    out_transition_id,
                                    in_transition_id,
                                    Some((out_transition_grounding_id, in_transition_grounding_id)),
                                )
                            })
                        })
                        .collect(),
                );
                res
            };

            let expr = if !encoding.includes_recovered_mies() && is_transition_eff(in_transition_id) {
                // In the case where we do not recover and use "missing" initial effects,
                // the inflow constraints for (all) effects are slightly weaker.
                RowExpr::new_geq_single_lhs(terms)
            } else {
                RowExpr::new_eq_single_lhs(terms)
            };

            problem.push_row(expr);
        }

        if !encoding.includes_recovered_mies() {
            // In the case where we do not recover and use "missing" initial effects,
            // the inflow constraints for (all) effects are slightly weaker (see above).
            // This is (partially? FIXME[proof?]) compensated by the following constraints,
            // which state that the *sum* of inflows into (ground effects) with the *same state variable* (so, independent of their value) is upper bounded by 1.

            let chunkby = groundings
                .supports()
                .iter_in_all()
                .map(
                    |(in_transition_id, in_transition_grounding_id, out_transitions_groundings)| {
                        (in_transition_grounding_id, in_transition_id, out_transitions_groundings)
                    },
                )
                .sorted_unstable_by_key(|&(in_transition_grounding_id, _, _)| *in_transition_grounding_id)
                .chunk_by(|&(in_transition_grounding_id, _, _)| in_transition_grounding_id.state_var_grounding_id);

            for (_, x) in chunkby.into_iter() {
                let terms = x
                    .into_iter()
                    .filter_map(
                        |(&in_transition_grounding_id, &in_transition_id, out_transition_grounding)| {
                            out_transition_grounding.map(|(out_transition_id, out_transition_grounding_id)| {
                                ColTag::Support(
                                    out_transition_id,
                                    in_transition_id,
                                    Some((out_transition_grounding_id, in_transition_grounding_id)),
                                )
                            })
                        },
                    )
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
        let chunkby =
            groundings
                .supports()
                .iter_out_all()
                .chunk_by(|(out_transition_id, out_transition_grounding_id, _)| {
                    (out_transition_id, out_transition_grounding_id)
                });

        for ((&out_transition_id, &out_transition_grounding_id), in_transitions_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::PresenceTransition(
                    out_transition_id,
                    Some(out_transition_grounding_id),
                )];
                res.append(
                    &mut in_transitions_groundings_ids
                        .into_iter()
                        .filter(|&&(_, _, (in_transition_id, _))| !is_transition_cond(in_transition_id))
                        .map(|&(_, _, (in_transition_id, in_transition_grounding_id))| {
                            ColTag::Support(
                                out_transition_id,
                                in_transition_id,
                                Some((out_transition_grounding_id, in_transition_grounding_id)),
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
    //     for &(out_transition_id, in_transition_id, transitions_groundings_ids) in ground_supports_iter_sorted_fully {
    //         let Some((out_transition_grounding_id, in_transition_grounding_id)) = transitions_groundings_ids else {
    //             continue;
    //         };
    //
    //         seen.push((
    //             out_transition_id,
    //             in_transition_id,
    //             out_transition_grounding_id,
    //             in_transition_grounding_id,
    //         ));
    //
    //         if seen
    //             .binary_search(&(
    //                 in_transition_id,
    //                 out_transition_id,
    //                 in_transition_grounding_id,
    //                 out_transition_grounding_id,
    //             ))
    //             .is_ok()
    //         {
    //             let expr = RowExpr::Leq1(vec![
    //                 ColTag::SupportGround(
    //                     out_transition_id,
    //                     in_transition_id,
    //                     out_transition_grounding_id,
    //                     in_transition_grounding_id,
    //                 ),
    //                 ColTag::SupportGround(
    //                     in_transition_id,
    //                     out_transition_id,
    //                     in_transition_grounding_id,
    //                     out_transition_grounding_id,
    //                 ),
    //             ]);
    //             problem.push_row(expr);
    //         }
    //     }
    //     debug_assert!(seen.is_sorted());
    // }

    // [Ground] At most one of a term's groundings can be active
    {
        let chunkby = groundings
            .terms()
            .iter_sorted_all_only_assignments()
            .chunk_by(|&(term, _)| term);

        for (term, values) in chunkby.into_iter() {
            let terms = values
                .into_iter()
                .map(|(_, value)| ColTag::TermGround(term, value))
                .collect::<Vec<_>>();

            debug_assert!(!terms.is_empty());

            let expr = RowExpr::new_leq_1(terms);
            problem.push_row(expr);
        }
    }

    // [Ground] A grounding of a term is active iff a ground transition using it is active
    // [Ground] ---------------------------------------------- source --------------------
    {
        let chunkby = groundings
            .terms()
            .iter_sorted_all_for_transitions()
            .chunk_by(|&(term, value, transition_id, _)| (term, value, transition_id));

        for ((&term, &value, &transition_id), transitions_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::TermGround(term, value)];
                res.append(
                    &mut transitions_groundings_ids
                        .into_iter()
                        .map(|&(_, _, _, transition_grounding_id)| {
                            ColTag::PresenceTransition(transition_id, Some(transition_grounding_id))
                        })
                        .collect(),
                );
                res
            };
            debug_assert!(terms.len() >= 2);

            debug_assert!(ctx.store.state.implies(
                encoding.get_transition_prez(transition_id, ctx),
                ctx.store.presence(term)
            ));
            let presences_equivalent = ctx.store.state.implies(
                ctx.store.presence(term),
                encoding.get_transition_prez(transition_id, ctx),
            );

            let expr = if presences_equivalent {
                RowExpr::new_eq_single_lhs(terms)
            } else {
                RowExpr::new_geq_single_lhs(terms)
            };
            problem.push_row(expr);
        }

        let chunkby = groundings
            .terms()
            .iter_sorted_all_for_sources()
            .chunk_by(|&(term, value, source, _)| (term, value, source));

        for ((&term, &value, &source), sources_groundings_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::TermGround(term, value)];
                res.append(
                    &mut sources_groundings_ids
                        .into_iter()
                        .map(|&(_, _, _, source_grounding_id)| {
                            ColTag::PresenceSource(source, Some(source_grounding_id))
                        })
                        .collect(),
                );
                res
            };
            debug_assert!(terms.len() >= 2);

            debug_assert!(
                ctx.store
                    .state
                    .implies(encoding.get_source_prez(source, ctx), ctx.store.presence(term))
                    && ctx
                        .store
                        .state
                        .implies(ctx.store.presence(term), encoding.get_source_prez(source, ctx))
            );

            let expr = RowExpr::new_eq_single_lhs(terms);
            problem.push_row(expr);
        }
    }
}
