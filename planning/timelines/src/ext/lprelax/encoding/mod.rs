mod groundings;
mod lifted;

use std::collections::HashSet;

use aries_solver::core::IntCst;
use aries_solver::core::views::Dom;
use itertools::Itertools;

use crate::IntTerm;
use crate::encoder::SchedEncoder;
use crate::ext::lprelax::LpRelaxEncoder;
use crate::ext::lprelax::encoding::groundings::{LpRelaxEncodingGroundingsInfo, StateVarGroundingId};
use crate::ext::lprelax::encoding::lifted::LpRelaxEncodingLiftedSupportsSorted;
use crate::ext::lprelax::transitions::{TransitionGroundingId, TransitionId};
use crate::ext::{Source, SourceGrounding, SourceGroundingId};

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub(crate) enum ColTag {
    PresenceSource(Source, Option<SourceGroundingId>),
    PresenceTransition(TransitionId, Option<TransitionGroundingId>),
    Support(
        TransitionId,
        TransitionId,
        Option<(TransitionGroundingId, TransitionGroundingId)>,
    ),
    SupportInitialGroundClosedWorld(StateVarGroundingId, TransitionId, IntCst),
    TermGround(IntTerm, IntCst),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum RowExprType {
    Eq,
    Leq,
    Geq,
}

/// lhs cmp rhs + cst
/// where cmp can be =, <=, or >=, and terms in lhs and rhs have unit coefficients (1).
#[derive(Debug, Clone)]
pub(crate) struct RowExpr {
    pub tpe: RowExprType,
    lhs_rhs_limit: usize,
    terms: Vec<ColTag>,
    cst: IntCst,
}
impl RowExpr {
    pub fn lhs(&self) -> &[ColTag] {
        self.terms.get(..self.lhs_rhs_limit).unwrap_or(&[])
    }
    pub fn rhs(&self) -> &[ColTag] {
        self.terms.get(self.lhs_rhs_limit..).unwrap_or(&[])
    }
    pub fn cst(&self) -> IntCst {
        self.cst
    }
    #[allow(dead_code)]
    pub fn new(tpe: RowExprType, terms: Vec<ColTag>, lhs_rhs_limit: usize, cst: IntCst) -> Self {
        Self {
            tpe,
            lhs_rhs_limit,
            terms,
            cst,
        }
    }
    pub fn new_eq_single_lhs(terms: Vec<ColTag>) -> Self {
        Self {
            tpe: RowExprType::Eq,
            lhs_rhs_limit: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_leq_single_lhs(terms: Vec<ColTag>) -> Self {
        Self {
            tpe: RowExprType::Leq,
            lhs_rhs_limit: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_geq_single_lhs(terms: Vec<ColTag>) -> Self {
        Self {
            tpe: RowExprType::Geq,
            lhs_rhs_limit: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_leq_1(terms: Vec<ColTag>) -> Self {
        Self {
            tpe: RowExprType::Leq,
            lhs_rhs_limit: terms.len(),
            terms,
            cst: 1,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct LpRelaxProblem {
    cols_set: HashSet<ColTag>,
    cols_vec: Vec<ColTag>,
    rows: Vec<RowExpr>,
}
impl LpRelaxProblem {
    pub fn insert_col(&mut self, col: ColTag) {
        if !self.cols_set.contains(&col) {
            self.cols_set.insert(col.clone());
            self.cols_vec.push(col);
        }
    }
    pub fn contains_col(&self, col: &ColTag) -> bool {
        self.cols_set.contains(col)
    }
    pub fn cols(&self) -> &[ColTag] {
        &self.cols_vec
    }
    pub fn push_row(&mut self, row: RowExpr) {
        self.rows.push(row);
    }
    pub fn rows(&self) -> &[RowExpr] {
        &self.rows
    }
}

#[derive(Clone, Default)]
pub(crate) struct LpRelaxEncoding {
    ready: bool,
    groundings: LpRelaxEncodingGroundingsInfo,
    lifted_supports_sorted: LpRelaxEncodingLiftedSupportsSorted,
}
impl LpRelaxEncoding {
    // Interns a grounding of a source.
    // WARNING: adding duplicate groundings (for the same source) will result in a panic (in debug mode).
    pub fn post_ground_source(
        &mut self,
        source: Source,
        source_grounding: SourceGrounding,
        encoder: &LpRelaxEncoder,
        ctx: &SchedEncoder,
    ) {
        self.ready = false;
        self.groundings
            .post_ground_source(source, source_grounding, encoder, ctx);
    }

    pub fn build(&mut self, encoder: &LpRelaxEncoder, ctx: &SchedEncoder) -> LpRelaxProblem {
        let time = std::time::Instant::now();
        println!("|- Lifted part encoding construction started");

        let mut problem = LpRelaxProblem::default();

        encode_problem_lifted(encoder, ctx, &mut self.lifted_supports_sorted, &mut problem);

        println!(
            "|- Lifted part encoding construction ended (run time: {})",
            time.elapsed().as_secs_f64()
        );

        let time = std::time::Instant::now();
        println!("|- Ground part encoding construction started");

        encode_problem_ground(
            encoder,
            ctx,
            &mut self.groundings,
            &mut self.lifted_supports_sorted,
            &mut problem,
        );

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

    pub fn iter_terms_assignments(&self) -> impl Iterator<Item = (crate::IntTerm, aries_solver::core::IntCst)> {
        self.groundings.terms().iter_sorted_all_only_assignments()
    }
}

fn encode_problem_lifted(
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    lifted_supports_sorted: &mut LpRelaxEncodingLiftedSupportsSorted,
    problem: &mut LpRelaxProblem,
) {
    *lifted_supports_sorted = LpRelaxEncodingLiftedSupportsSorted::from(encoder, ctx);

    // [Lifted] A source is present if(f) its transitions are
    // TODO: optimize iterations ? (flattened ?)
    {
        for source in encoder.iter_sources(ctx) {
            problem.insert_col(ColTag::PresenceSource(source, None));

            let source_prez = encoder
                .get_source(source, ctx)
                .map_or(aries_solver::core::Lit::TRUE, |t| t.presence);

            for &transition_id in encoder.transitions.of_source(source) {
                problem.insert_col(ColTag::PresenceTransition(transition_id, None));

                let transition_prez = encoder.transitions.get_prez(transition_id, ctx);

                // It is possible that sometimes the presence literal of the transition differs from that of the source
                // (in particular, as a result of complying with pddl set semantics, if the transition presence literal was replaced with a more specific one than the source's).
                // However, the latter must always imply the former.

                debug_assert!(ctx.store.state.implies(transition_prez, source_prez));
                let presences_equivalent = ctx.store.state.implies(source_prez, transition_prez);

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

    let is_transition_eff = |transition_id| encoder.transitions.is_pure_eff(transition_id);
    let is_transition_cond = |transition_id| encoder.transitions.is_pure_cond(transition_id);

    // [Lifted] Support between two transitions implies presence of both of them
    // NOTE: There's no need to enforce theses constraints for all cases, as the inflow and outflow constraints are stronger (see below).
    //       They're only actually needed for out-conditions when the in-transition is a "pure-condition", as this case is not implied by outflow constraints.
    {
        for &(out_transition_id, in_transition_id) in lifted_supports_sorted.out() {
            problem.insert_col(ColTag::Support(out_transition_id, in_transition_id, None));

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

        for &(out_transition_id, in_transition_id) in lifted_supports_sorted.out() {
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
            debug_assert!(terms.iter().all(|col_tag| problem.contains_col(col_tag)));
            debug_assert!(terms.len() >= 2);

            let expr = if !encoder.transitions.includes_recovered_mies() && is_transition_eff(in_transition_id) {
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
            .chunk_by(|&(out_transition_id, _)| out_transition_id);

        for (&out_transition_id, in_transitions_ids) in chunkby.into_iter() {
            let terms = {
                let mut res = vec![ColTag::PresenceTransition(out_transition_id, None)];
                res.append(
                    &mut in_transitions_ids
                        .into_iter()
                        .filter(|&&(_, in_transition_id)| !is_transition_cond(in_transition_id))
                        .map(|&(_, in_transition_id)| ColTag::Support(out_transition_id, in_transition_id, None))
                        .collect(),
                );
                res
            };
            debug_assert!(terms.iter().all(|col_tag| problem.contains_col(col_tag)));

            if terms.len() >= 2 {
                let expr = RowExpr::new_geq_single_lhs(terms);
                problem.push_row(expr);
            }
        }
    }
}

fn encode_problem_ground(
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    groundings: &mut LpRelaxEncodingGroundingsInfo,
    lifted_supports_sorted: &mut LpRelaxEncodingLiftedSupportsSorted,
    problem: &mut LpRelaxProblem,
) {
    // [Lifted-Ground] Source ground decomposition (a source is present iff one its groundings is)
    {
        for source in encoder.iter_sources(ctx) {
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
            for col_tag in &terms {
                problem.insert_col(col_tag.clone());
            }
            if terms.len() >= 2 {
                let expr = RowExpr::new_eq_single_lhs(terms);
                problem.push_row(expr);
            } else {
                // ? WARNING ? related to incomplete / partial groundings. [TODO]
                continue;
            }
        }
    }

    groundings.transitions_sort();

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
            for col_tag in &terms {
                problem.insert_col(col_tag.clone());
            }
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

            let source_prez = encoder
                .get_source(source, ctx)
                .map_or(aries_solver::core::Lit::TRUE, |t| t.presence);

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
                debug_assert!(terms.iter().all(|col_tag| problem.contains_col(col_tag)));
                debug_assert!(terms.len() >= 2);

                let transition_prez = encoder.transitions.get_prez(transition_id, ctx);

                // Same as for the lifted case:
                // It is possible that sometimes the presence literal of the transition differs from that of the source
                // (in particular, as a result of complying with pddl set semantics, if the transition presence literal was replaced with a more specific one than the source's).
                // However, the latter must always imply the former.

                debug_assert!(ctx.store.state.implies(transition_prez, source_prez));
                let presences_equivalent = ctx.store.state.implies(source_prez, transition_prez);

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

    let is_transition_eff = |transition_id| encoder.transitions.is_pure_eff(transition_id);
    let is_transition_cond = |transition_id| encoder.transitions.is_pure_cond(transition_id);

    let time = std::time::Instant::now();
    println!("|--- Ground part encoding construction: support building started");

    groundings.supports_build(lifted_supports_sorted /*is_transition_eff*/);

    println!(
        "|--- Ground part encoding construction: support building ended (run time {})",
        time.elapsed().as_secs_f64()
    );

    // [Ground] Support between two (ground) transitions implies presence of both of them
    // NOTE: There's no need to enforce theses constraints for all cases, as the (ground) inflow and outflow constraints are stronger (see below).
    //       They're only actually needed for out-conditions when the in-transition is a "pure-condition", as this case is not implied by (ground) outflow constraints.
    {
        for &(out_transition_id, in_transition_id, transition_groundings_ids) in groundings.supports().iter_all() {
            let Some((out_transition_grounding_id, in_transition_grounding_id)) = transition_groundings_ids else {
                continue;
            };

            problem.insert_col(ColTag::Support(
                out_transition_id,
                in_transition_id,
                Some((out_transition_grounding_id, in_transition_grounding_id)),
            ));

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
            for col_tag in &terms {
                problem.insert_col(col_tag.clone());
            }

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
            debug_assert!(terms.iter().all(|col_tag| problem.contains_col(col_tag)));

            let expr = if !encoder.transitions.includes_recovered_mies() && is_transition_eff(in_transition_id) {
                // In the case where we do not recover and use "missing" initial effects,
                // the inflow constraints for (all) effects are slightly weaker.
                RowExpr::new_geq_single_lhs(terms)
            } else {
                RowExpr::new_eq_single_lhs(terms)
            };

            problem.push_row(expr);
        }

        if !encoder.transitions.includes_recovered_mies() {
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
            debug_assert!(terms.iter().all(|col_tag| problem.contains_col(col_tag)));

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

    groundings.terms_sort();

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

            for col_tag in &terms {
                problem.insert_col(col_tag.clone());
            }
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
            debug_assert!(terms.iter().all(|col_tag| problem.contains_col(col_tag)));
            debug_assert!(problem.contains_col(&ColTag::TermGround(term, value)));
            debug_assert!(terms.len() >= 2);

            let transition_prez = encoder.transitions.get_prez(transition_id, ctx);

            debug_assert!(ctx.store.state.implies(transition_prez, ctx.store.presence(term)));
            let presences_equivalent = ctx.store.state.implies(ctx.store.presence(term), transition_prez);

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
            debug_assert!(terms.iter().all(|col_tag| problem.contains_col(col_tag)));
            debug_assert!(terms.len() >= 2);

            let source_prez = encoder
                .get_source(source, ctx)
                .map_or(aries_solver::core::Lit::TRUE, |t| t.presence);

            debug_assert!(
                ctx.store.state.implies(source_prez, ctx.store.presence(term))
                    && ctx.store.state.implies(ctx.store.presence(term), source_prez)
            );

            let expr = RowExpr::new_eq_single_lhs(terms);
            problem.push_row(expr);
        }
    }
}
