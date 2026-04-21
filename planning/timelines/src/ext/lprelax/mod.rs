use std::collections::{BTreeMap, BTreeSet};

use aries::core::views::Term;
use idmap::intid::IntegerId;
use itertools::Itertools;

use aries::model::lang::BoolExpr;
use aries::prelude::{Conjunction, IntCst, Lit};
use aries::utils::StreamingIterator;

use aries_lprelax::*;

use crate::encoder::SchedEncoder;
use crate::ext::{
    SchedEncoderExt, SourceGroundingIdFlat, SourceId, TermGroundingId, Transition, TransitionGroundingIdFlat,
    TransitionId, iter_source_groundings, term_value_from_id,
};
use crate::{IntTerm, TaskId};

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
enum ColTag {
    PresenceSource(SourceId),
    PresenceSourceGround(SourceId, SourceGroundingIdFlat),
    PresenceTransition(TransitionId),
    PresenceTransitionGround(TransitionId, TransitionGroundingIdFlat),
    CausalLink(TransitionId, TransitionId),
    CausalLinkGround(
        TransitionId,
        TransitionId,
        TransitionGroundingIdFlat,
        TransitionGroundingIdFlat,
    ),
    TermGround(IntTerm, TermGroundingId),
}

type Presences = Vec<(SourceId, TransitionId)>;
type PresencesTransitionGroundDecomp = BTreeMap<TransitionId, BTreeSet<TransitionGroundingIdFlat>>;
type PresencesTransitionGroundDecompSources =
    BTreeMap<(TransitionId, TransitionGroundingIdFlat), BTreeSet<SourceGroundingIdFlat>>;
type PresencesSourcesGroundDecomp = BTreeMap<SourceId, BTreeSet<SourceGroundingIdFlat>>;
type CausalLinks = Vec<(TransitionId, TransitionId)>; // BTreeMap<(TransitionId, TransitionId), Lit>;
type CausalLinksGroundDecomp =
    BTreeMap<(TransitionId, TransitionId), BTreeSet<(TransitionGroundingIdFlat, TransitionGroundingIdFlat)>>;
type CausalLinksInflow = BTreeMap<TransitionId, BTreeSet<TransitionId>>;
type CausalLinksInflowGround =
    BTreeMap<(TransitionId, TransitionGroundingIdFlat), BTreeMap<TransitionId, BTreeSet<TransitionGroundingIdFlat>>>;
type CausalLinksOutflowPure = BTreeMap<TransitionId, BTreeSet<TransitionId>>;
type CausalLinksOutflowPureGround =
    BTreeMap<(TransitionId, TransitionGroundingIdFlat), BTreeMap<TransitionId, BTreeSet<TransitionGroundingIdFlat>>>;
type IntTermsGroundDecompLeq1 = BTreeMap<IntTerm, BTreeSet<TermGroundingId>>;
type IntTermsGroundDecompSources = BTreeMap<(IntTerm, SourceId, SourceGroundingIdFlat), BTreeSet<TermGroundingId>>;
type IntTermsGroundDecompTransitions =
    BTreeMap<(IntTerm, TransitionId, TransitionGroundingIdFlat), BTreeSet<TermGroundingId>>;

#[derive(Debug)]
pub(crate) struct LpRelaxEncoding;

impl BoolExpr<SchedEncoder> for LpRelaxEncoding {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoder) {
        assert!(l.tautological());

        // TODO: assert this being the last constraint encoded ?
        // (equivalently, all other constraints already having been encoded ?)

        ctx.ext = Some(SchedEncoderExt::new(ctx));

        let mut presences = Presences::new();
        let mut presences_transition_ground_decomp = PresencesTransitionGroundDecomp::new();
        let mut presences_transition_ground_decomp_sources = PresencesTransitionGroundDecompSources::new();
        let mut presences_sources_ground_decomp = PresencesSourcesGroundDecomp::new();
        let mut causal_links = CausalLinks::new();
        let mut causal_links_ground_decomp = CausalLinksGroundDecomp::new();
        let mut causal_links_inflow = CausalLinksInflow::new();
        let mut causal_links_inflow_ground = CausalLinksInflowGround::new();
        let mut causal_links_outflow_pure = CausalLinksOutflowPure::new();
        let mut causal_links_outflow_pure_ground = CausalLinksOutflowPureGround::new();
        let mut terms_ground_decomp_leq1 = IntTermsGroundDecompLeq1::new();
        let mut terms_ground_decomp_sources = IntTermsGroundDecompSources::new();
        let mut terms_ground_decomp_transitions = IntTermsGroundDecompTransitions::new();

        let src_ids = [None].into_iter().chain(
            ctx.sched
                .tasks
                .iter()
                .enumerate()
                .map(|(task_id, _task)| Some(TaskId::from_int(u32::try_from(task_id).unwrap()))),
        );

        for src_id in src_ids {
            let Some(tr_ids) = ctx.ext.as_ref().unwrap().transitions.of_source(&src_id) else {
                continue;
            };

            for &tr_id in tr_ids {
                presences.push((src_id, tr_id));

                let mut src_grs = iter_source_groundings(src_id, ctx);
                while let Some(src_gr) = src_grs.next() {
                    let src_gr_idflat = src_gr.id_flat();

                    presences_sources_ground_decomp
                        .entry(src_id)
                        .or_default()
                        .insert(src_gr_idflat);

                    for term_gr in src_gr.to_term_groundings(ctx) {
                        terms_ground_decomp_sources
                            .entry((term_gr.term, src_id, src_gr_idflat))
                            .or_default()
                            .insert(term_gr.id);

                        terms_ground_decomp_leq1
                            .entry(term_gr.term)
                            .or_default()
                            .insert(term_gr.id);
                    }

                    let Some(trs_grs) = src_gr.get_transitions_groundings(ctx) else {
                        continue;
                    };
                    for tr_gr in trs_grs {
                        let tr_gr_idflat = tr_gr.id_flat();

                        presences_transition_ground_decomp
                            .entry(tr_id)
                            .or_default()
                            .insert(tr_gr_idflat);

                        presences_transition_ground_decomp_sources
                            .entry((tr_id, tr_gr_idflat))
                            .or_default()
                            .insert(src_gr_idflat);

                        for term_gr in tr_gr.to_term_groundings(ctx) {
                            terms_ground_decomp_transitions
                                .entry((term_gr.term, tr_id, tr_gr_idflat))
                                .or_default()
                                .insert(term_gr.id);

                            terms_ground_decomp_leq1
                                .entry(term_gr.term)
                                .or_default()
                                .insert(term_gr.id);
                        }
                    }
                }
            }
        }

        for ((tr2_id, tr2), src2_id) in ctx.ext.as_ref().unwrap().transitions.iter() {
            let tr2_is_init = src2_id.is_none() && matches!(tr2, Transition::Eff(_));

            if tr2_is_init {
                continue;
            }
            let tr1_ids = gather_candidate_supporter_transitions(&tr2, ctx);

            for &tr1_id in &tr1_ids {
                let src1_id = ctx.ext.as_ref().unwrap().transitions.store[tr1_id].get_source(ctx);
                let tr1_is_final = src1_id.is_none() && matches!(tr2, Transition::Cond(_));

                if tr1_is_final {
                    continue;
                }
                causal_links.push((tr1_id, tr2_id));

                causal_links_inflow.entry(tr2_id).or_default().insert(tr1_id);

                if matches!(tr2, Transition::CondEff(_, _)) {
                    causal_links_outflow_pure.entry(tr1_id).or_default().insert(tr2_id);
                }

                let mut src1_grs = iter_source_groundings(src1_id, ctx);
                while let Some(src1_gr) = src1_grs.next() {
                    let tr1_gr_idflat = src1_gr.get_transition_grounding(tr1_id, ctx).unwrap().id_flat();
                    let tr1_gr_asgt = src1_gr.get_transition_grounding(tr1_id, ctx).unwrap().assignment;

                    let mut src2_grs = iter_source_groundings(src2_id, ctx);
                    while let Some(src2_gr) = src2_grs.next() {
                        let tr2_gr_idflat = src2_gr.get_transition_grounding(tr2_id, ctx).unwrap().id_flat();
                        let tr2_gr_asgt = src2_gr.get_transition_grounding(tr2_id, ctx).unwrap().assignment;

                        if !is_valid_causal_link_grounding(tr1_id, tr2_id, &tr1_gr_asgt, &tr2_gr_asgt, ctx) {
                            continue;
                        }

                        causal_links_ground_decomp
                            .entry((tr1_id, tr2_id))
                            .or_default()
                            .insert((tr1_gr_idflat, tr2_gr_idflat));

                        causal_links_inflow_ground
                            .entry((tr2_id, tr2_gr_idflat))
                            .or_default()
                            .entry(tr1_id)
                            .or_default()
                            .insert(tr1_gr_idflat);

                        if matches!(tr2, Transition::CondEff(_, _)) {
                            causal_links_outflow_pure_ground
                                .entry((tr1_id, tr1_gr_idflat))
                                .or_default()
                                .entry(tr2_id)
                                .or_default()
                                .insert(tr2_gr_idflat);
                        }
                    }
                }
            }
        }

        let mut col_tags = BTreeMap::<ColTag, LpCol>::new();
        let mut lprelax = LpRelax::default();

        for &(src_id, tr_id) in &presences {
            col_tags
                .entry(ColTag::PresenceTransition(tr_id))
                .or_insert_with(|| lprelax.add_column_01());

            col_tags.entry(ColTag::PresenceSource(src_id)).or_insert_with(|| {
                if src_id.is_none() {
                    lprelax.add_column_01()
                } else {
                    lprelax.add_column(Some(1.), Some(1.))
                }
            });
        }

        for (&src_id, src_grs) in &presences_sources_ground_decomp {
            for &src_gr in src_grs {
                col_tags
                    .entry(ColTag::PresenceSourceGround(src_id, src_gr))
                    .or_insert_with(|| lprelax.add_column_01());
            }
            let row_coefs = src_grs
                .iter()
                .map(|&src_gr| {
                    (
                        *col_tags.get(&ColTag::PresenceSourceGround(src_id, src_gr)).unwrap(),
                        1.,
                    )
                })
                .chain([(*col_tags.get(&ColTag::PresenceSource(src_id)).unwrap(), -1.)]);

            lprelax.add_row(row_coefs, Some(0.), Some(0.));
        }

        for (&tr_id, tr_grs) in &presences_transition_ground_decomp {
            for &tr_gr in tr_grs {
                col_tags
                    .entry(ColTag::PresenceTransitionGround(tr_id, tr_gr))
                    .or_insert_with(|| lprelax.add_column_01());
            }
            let row_coefs = tr_grs
                .iter()
                .map(|&tr_gr| {
                    (
                        *col_tags.get(&ColTag::PresenceTransitionGround(tr_id, tr_gr)).unwrap(),
                        1.,
                    )
                })
                .chain([(*col_tags.get(&ColTag::PresenceTransition(tr_id)).unwrap(), -1.)]);

            lprelax.add_row(row_coefs, Some(0.), Some(0.));
        }

        for (&(tr_id, tr_gr), src_grs) in &presences_transition_ground_decomp_sources {
            let src_id = ctx.ext.as_ref().unwrap().transitions.store[tr_id].get_source(ctx);

            let row_coefs = src_grs
                .iter()
                .map(|&src_gr| {
                    (
                        *col_tags.get(&ColTag::PresenceSourceGround(src_id, src_gr)).unwrap(),
                        1.,
                    )
                })
                .chain([(
                    *col_tags.get(&ColTag::PresenceTransitionGround(tr_id, tr_gr)).unwrap(),
                    -1.,
                )]);

            lprelax.add_row(row_coefs, Some(0.), Some(0.));
        }

        for (&term, term_grs) in &terms_ground_decomp_leq1 {
            for &term_gr in term_grs {
                col_tags
                    .entry(ColTag::TermGround(term, term_gr))
                    .or_insert_with(|| lprelax.add_column_01());
            }
            let row_coefs = term_grs
                .iter()
                .map(|&term_gr| (*col_tags.get(&ColTag::TermGround(term, term_gr)).unwrap(), 1.));

            lprelax.add_row(row_coefs, None, Some(1.));
        }

        for (&(term, src_id, src_gr), term_grs) in &terms_ground_decomp_sources {
            let row_coefs = term_grs
                .iter()
                .map(|&term_gr| (*col_tags.get(&ColTag::TermGround(term, term_gr)).unwrap(), 1.))
                .chain([(
                    *col_tags.get(&ColTag::PresenceSourceGround(src_id, src_gr)).unwrap(),
                    -1.,
                )]);

            lprelax.add_row(row_coefs, Some(0.), Some(0.));
        }

        for (&(term, tr_id, tr_gr), term_grs) in &terms_ground_decomp_transitions {
            let row_coefs = term_grs
                .iter()
                .map(|&term_gr| (*col_tags.get(&ColTag::TermGround(term, term_gr)).unwrap(), 1.))
                .chain([(
                    *col_tags.get(&ColTag::PresenceTransitionGround(tr_id, tr_gr)).unwrap(),
                    -1.,
                )]);

            lprelax.add_row(row_coefs, Some(0.), Some(0.));
        }

        for &(tr1_id, tr2_id) in &causal_links {
            col_tags
                .entry(ColTag::CausalLink(tr1_id, tr2_id))
                .or_insert_with(|| lprelax.add_column_01());

            lprelax.add_row(
                [
                    (*col_tags.get(&ColTag::CausalLink(tr1_id, tr2_id)).unwrap(), 1.),
                    (*col_tags.get(&ColTag::PresenceTransition(tr1_id)).unwrap(), -1.),
                ],
                None,
                Some(0.),
            );
            lprelax.add_row(
                [
                    (*col_tags.get(&ColTag::CausalLink(tr1_id, tr2_id)).unwrap(), 1.),
                    (*col_tags.get(&ColTag::PresenceTransition(tr2_id)).unwrap(), -1.),
                ],
                None,
                Some(0.),
            );
        }

        for (&(tr1_id, tr2_id), trs_grs) in &causal_links_ground_decomp {
            for &(tr1_gr, tr2_gr) in trs_grs {
                col_tags
                    .entry(ColTag::CausalLinkGround(tr1_id, tr2_id, tr1_gr, tr2_gr))
                    .or_insert_with(|| lprelax.add_column_01());

                lprelax.add_row(
                    [
                        (
                            *col_tags
                                .get(&ColTag::CausalLinkGround(tr1_id, tr2_id, tr1_gr, tr2_gr))
                                .unwrap(),
                            1.,
                        ),
                        (
                            *col_tags.get(&ColTag::PresenceTransitionGround(tr1_id, tr1_gr)).unwrap(),
                            -1.,
                        ),
                    ],
                    None,
                    Some(0.),
                );
                lprelax.add_row(
                    [
                        (
                            *col_tags
                                .get(&ColTag::CausalLinkGround(tr1_id, tr2_id, tr1_gr, tr2_gr))
                                .unwrap(),
                            1.,
                        ),
                        (
                            *col_tags.get(&ColTag::PresenceTransitionGround(tr2_id, tr2_gr)).unwrap(),
                            -1.,
                        ),
                    ],
                    None,
                    Some(0.),
                );
            }
            let row_coefs = trs_grs
                .iter()
                .map(|&(tr1_gr, tr2_gr)| {
                    (
                        *col_tags
                            .get(&ColTag::CausalLinkGround(tr1_id, tr2_id, tr1_gr, tr2_gr))
                            .unwrap(),
                        1.,
                    )
                })
                .chain([(*col_tags.get(&ColTag::CausalLink(tr1_id, tr2_id)).unwrap(), -1.)]);

            lprelax.add_row(row_coefs, Some(0.), Some(0.));
        }

        for (&tr2_id, tr1_ids) in &causal_links_inflow {
            let row_coefs = tr1_ids
                .iter()
                .map(|&tr1_id| (*col_tags.get(&ColTag::CausalLink(tr1_id, tr2_id)).unwrap(), 1.))
                .chain([(*col_tags.get(&ColTag::PresenceTransition(tr2_id)).unwrap(), -1.)]);

            lprelax.add_row(row_coefs, Some(0.), Some(0.));
        }

        for (&(tr2_id, tr2_gr), tr1s) in &causal_links_inflow_ground {
            let row_coefs = tr1s
                .iter()
                .flat_map(|(tr1_id, tr1_grs)| {
                    tr1_grs.iter().map(|tr1_gr| {
                        (
                            *col_tags
                                .get(&ColTag::CausalLinkGround(*tr1_id, tr2_id, *tr1_gr, tr2_gr))
                                .unwrap(),
                            1.,
                        )
                    })
                })
                .chain([(
                    *col_tags.get(&ColTag::PresenceTransitionGround(tr2_id, tr2_gr)).unwrap(),
                    -1.,
                )]);

            lprelax.add_row(row_coefs, Some(0.), Some(0.));
        }

        for (&tr1_id, tr2_ids) in &causal_links_outflow_pure {
            let row_coefs = tr2_ids
                .iter()
                .map(|&tr2_id| (*col_tags.get(&ColTag::CausalLink(tr1_id, tr2_id)).unwrap(), 1.))
                .chain([(*col_tags.get(&ColTag::PresenceTransition(tr1_id)).unwrap(), -1.)]);

            lprelax.add_row(row_coefs, None, Some(0.));
        }

        for (&(tr1_id, tr1_gr), tr2s) in &causal_links_outflow_pure_ground {
            let row_coefs = tr2s
                .iter()
                .flat_map(|(tr2_id, tr2_grs)| {
                    tr2_grs.iter().map(|tr2_gr| {
                        (
                            *col_tags
                                .get(&ColTag::CausalLinkGround(tr1_id, *tr2_id, tr1_gr, *tr2_gr))
                                .unwrap(),
                            1.,
                        )
                    })
                })
                .chain([(
                    *col_tags.get(&ColTag::PresenceTransitionGround(tr1_id, tr1_gr)).unwrap(),
                    -1.,
                )]);

            lprelax.add_row(row_coefs, None, Some(0.));
        }

        for &(src_id, tr_id) in &presences {
            let p = ctx.ext.as_ref().unwrap().transitions.store[tr_id]
                .get_prez(ctx)
                .variable();
            let col = *col_tags.get(&ColTag::PresenceTransition(tr_id)).unwrap();

            lprelax.register_lit_implier(p, new_default_lit_implier(p, col));
            lprelax.register_lplit_implier(col, new_default_lplit_implier(p, col));

            if let Some(src_id) = src_id {
                let p = ctx.sched.tasks[src_id].presence.variable();
                let col = *col_tags.get(&ColTag::PresenceSource(Some(src_id))).unwrap();

                lprelax.register_lit_implier(p, new_default_lit_implier(p, col));
                lprelax.register_lplit_implier(col, new_default_lplit_implier(p, col));
            }
        }

        for (&term, term_grs) in &terms_ground_decomp_leq1 {
            let var = term.variable();

            let mappings: Vec<(LpCol, IntCst)> = term_grs
                .iter()
                .map(|&term_gr| {
                    let val = term_value_from_id(&IntTerm::from(var), term_gr.0, ctx).unwrap();
                    let col = *col_tags.get(&ColTag::TermGround(term, term_gr)).unwrap();
                    (col, val)
                })
                .collect();

            lprelax.register_lit_implier(
                var, // TODO FIXME: change varref to term in lprelax reasoner!
                std::sync::Arc::new(move |lit: Lit| {
                    assert_eq!(lit.variable(), var);
                    let vec = mappings
                        .iter()
                        .filter_map(|&(col, val)| {
                            (lit.entails(var.lt(val)) || lit.entails(var.gt(val))).then_some(LpLit::leq(col, 0))
                        })
                        .collect_vec();
                    if !vec.is_empty() { Some(vec.into()) } else { None }
                }),
            );

            for &term_gr in term_grs {
                //let val = term.value_for_index(term_gr.0, ctx).unwrap(); // TODO FIXME: change varref to term in lprelax reasoner!
                let val = term_value_from_id(&IntTerm::from(term.variable()), term_gr.0, ctx).unwrap();
                let col = *col_tags.get(&ColTag::TermGround(term, term_gr)).unwrap();

                lprelax.register_lplit_implier(
                    col, // TODO FIXME: change varref to term in lprelax reasoner!
                    std::sync::Arc::new(move |lplit: LpLit| {
                        assert_eq!(lplit.col, col);
                        Some(smallvec::smallvec![var.geq(val), var.leq(val)])
                    }),
                );
            }
        }

        for &(tr1_id, tr2_id) in &causal_links {
            let col = *col_tags.get(&ColTag::CausalLink(tr1_id, tr2_id)).unwrap();
            if let Some(s) = match (
                ctx.ext.as_ref().unwrap().transitions.store[tr1_id],
                ctx.ext.as_ref().unwrap().transitions.store[tr2_id],
            ) {
                (Transition::Eff(eid), Transition::Cond(cid)) => {
                    Some(ctx.causal_links.store[eid].get(&cid).unwrap().variable())
                }
                (Transition::Eff(eid), Transition::CondEff(cid, _)) => {
                    Some(ctx.causal_links.store[eid].get(&cid).unwrap().variable())
                }
                (Transition::CondEff(_, eid), Transition::Cond(cid)) => {
                    Some(ctx.causal_links.store[eid].get(&cid).unwrap().variable())
                }
                (Transition::CondEff(_, eid), Transition::CondEff(cid, _)) => {
                    Some(ctx.causal_links.store[eid].get(&cid).unwrap().variable())
                }
                (Transition::Eff(_), Transition::Eff(_)) => None,
                (Transition::CondEff(_, _), Transition::Eff(_)) => None,
                _ => unreachable!(),
            } {
                lprelax.register_lit_implier(s, new_default_lit_implier(s, col));
                lprelax.register_lplit_implier(col, new_default_lplit_implier(s, col));
            }
        }
    }

    fn conj_scope(&self, _ctx: &SchedEncoder) -> Conjunction {
        Conjunction::tautology()
    }
}

fn gather_candidate_supporter_transitions(tr_to_supp: &Transition, ctx: &SchedEncoder) -> Vec<TransitionId> {
    match tr_to_supp {
        Transition::Cond(cid2) | Transition::CondEff(cid2, _) => ctx
            .causal_links
            .store
            .get(cid2)
            .map(|es| es.keys().copied().collect_vec())
            .unwrap_or_default(),
        Transition::Eff(eid2) => ctx
            .sched
            .effects
            .iter()
            .enumerate()
            .filter(|(eid1, e1)| {
                *eid1 != *eid2
                    //&& e1.source != tr_to_supp.get_source(ctx) // FIXME?
                    && e1.state_var == *tr_to_supp.get_state_var(ctx)
            })
            .map(|(eid1, _)| *ctx.ext.as_ref().unwrap().transitions.of_effect.get(eid1).unwrap())
            .collect_vec(),
    }
}

fn is_valid_causal_link_grounding(
    tr1_id: TransitionId,
    tr2_id: TransitionId,
    tr1_gr: &[IntCst],
    tr2_gr: &[IntCst],
    ctx: &SchedEncoder,
) -> bool {
    let tr1 = ctx.ext.as_ref().unwrap().transitions.store[tr1_id];
    let tr2 = ctx.ext.as_ref().unwrap().transitions.store[tr2_id];

    let (n1, n2) = (tr1_gr.len(), tr2_gr.len());

    match (tr1, tr2) {
        (Transition::Eff(_), Transition::Cond(_)) => *tr1_gr == *tr2_gr,
        (Transition::Eff(_), Transition::Eff(_)) => tr1_gr[..n1 - 1] == tr2_gr[..n2 - 1],
        (Transition::Eff(_), Transition::CondEff(_, _)) => *tr1_gr == tr2_gr[..n2 - 1],
        (Transition::CondEff(_, _), Transition::Cond(_)) => {
            tr1_gr[..n1 - 2] == tr2_gr[..n2 - 1] && tr1_gr[n1] == tr2_gr[n2]
        }
        (Transition::CondEff(_, _), Transition::Eff(_)) => tr1_gr[..n1 - 2] == tr2_gr[..n2 - 1],
        (Transition::CondEff(_, _), Transition::CondEff(_, _)) => {
            tr1_gr[..n1 - 2] == tr2_gr[..n2 - 2] && tr1_gr[n1] == tr2_gr[n2 - 1]
        }
        _ => unreachable!(),
    }
}
