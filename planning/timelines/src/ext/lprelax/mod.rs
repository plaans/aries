use std::collections::{BTreeMap, BTreeSet};

use aries::core::VarRef;
use aries::core::views::Term;
use idmap::intid::IntegerId;
use itertools::Itertools;

use aries::model::lang::BoolExpr;
use aries::prelude::{Conjunction, DomainsExt, IntCst, Lit};
use aries::utils::StreamingIterator;

use aries_lprelax::*;

use crate::ext::{
    SchedEncoderExt, SourceGrounding, SourceGroundingIdFlat, SourceId, TermGroundingId, Transition,
    TransitionGrounding, TransitionGroundingIdFlat, TransitionId,
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
type IntTermsGroundDecompSources = BTreeMap<(IntTerm, TermGroundingId), BTreeMap<SourceId, BTreeSet<SourceGroundingIdFlat>>>;
type IntTermsGroundDecompTransitions = BTreeMap<(IntTerm, TermGroundingId), BTreeMap<TransitionId, BTreeSet<TransitionGroundingIdFlat>>>;

#[derive(Debug)]
pub(crate) struct LpRelaxEncoding;

type Row = (Option<FloatCst>, Vec<(ColTag, FloatCst)>, Option<FloatCst>);

#[derive(Debug, Default)]
struct LpRelaxEncodingData {
    presences: Presences,
    presences_transition_ground_decomp: PresencesTransitionGroundDecomp,
    presences_transition_ground_decomp_sources: PresencesTransitionGroundDecompSources,
    presences_sources_ground_decomp: PresencesSourcesGroundDecomp,
    causal_links: CausalLinks,
    causal_links_ground_decomp: CausalLinksGroundDecomp,
    causal_links_inflow: CausalLinksInflow,
    causal_links_inflow_ground: CausalLinksInflowGround,
    causal_links_outflow_pure: CausalLinksOutflowPure,
    causal_links_outflow_pure_ground: CausalLinksOutflowPureGround,
    terms_ground_decomp_leq1: IntTermsGroundDecompLeq1,
    terms_ground_decomp_sources: IntTermsGroundDecompSources,
    terms_ground_decomp_transitions: IntTermsGroundDecompTransitions,

    col_tags: BTreeMap<ColTag, LpCol>,
    rows: Vec<Row>,
}

fn collect_candidate_supporters<'a>(tr2: &Transition, ctx: &'a SchedEncoderExt) -> Vec<(TransitionId, &'a Transition)> {
    match tr2 {
        Transition::Cond(c_id2) | Transition::CondEff(c_id2, _) => ctx
            .main
            .causal_links
            .store
            .get(c_id2)
            .map(|es| {
                es.keys()
                    .map(|eid1| ctx.transitions.get_for_effect(*eid1).unwrap())
                    .collect_vec()
            })
            .unwrap_or_default(),
        Transition::Eff(eid2) => ctx
            .main
            .sched
            .effects
            .iter()
            .enumerate()
            .filter(|(eid1, e1)| {
                *eid1 != *eid2
                && e1.source != tr2.get_source_id(&ctx.main) // FIXME?
                && e1.state_var.fluent == *tr2.get_state_var(&ctx.main).fluent
            })
            .map(|(eid1, _)| ctx.transitions.get_for_effect(eid1).unwrap())
            .collect_vec(),
    }
}

fn is_causal_link_grounding_valid(
    tr1: &Transition,
    tr1_gr: &TransitionGrounding,
    tr2: &Transition,
    tr2_gr: &TransitionGrounding,
) -> bool {
    let (assignment1, n1) = (&tr1_gr.assignment, tr1_gr.assignment.len());
    let (assignment2, n2) = (&tr2_gr.assignment, tr2_gr.assignment.len());

    match (tr1, tr2) {
        (Transition::Eff(_), Transition::Cond(_)) => assignment1 == assignment2,
        (Transition::Eff(_), Transition::Eff(_)) => assignment1[..n1 - 1] == assignment2[..n2 - 1],
        (Transition::Eff(_), Transition::CondEff(_, _)) => *assignment1 == assignment2[..n2 - 1],
        (Transition::CondEff(_, _), Transition::Cond(_)) => {
            assignment1[..n1 - 2] == assignment2[..n2 - 1] && assignment1[n1 - 1] == assignment2[n2 - 1]
        }
        (Transition::CondEff(_, _), Transition::Eff(_)) => assignment1[..n1 - 2] == assignment2[..n2 - 1],
        (Transition::CondEff(_, _), Transition::CondEff(_, _)) => {
            assignment1[..n1 - 2] == assignment2[..n2 - 2] && assignment1[n1 - 1] == assignment2[n2 - 2]
        }
        _ => unreachable!(),
    }
}

impl LpRelaxEncodingData {
    fn populate_tags(&mut self, ctx: &mut SchedEncoderExt) {
        let mut process_source_grounding = |src_gr: &SourceGrounding| -> SourceGroundingIdFlat {
            let src_gr_idflat = src_gr.idflat();

            self.presences_sources_ground_decomp
                .entry(src_gr.source_id)
                .or_default()
                .insert(src_gr_idflat);

            src_gr_idflat
        };
        let mut process_transition_grounding =
            |tr_gr: &TransitionGrounding, src_gr_idflat: SourceGroundingIdFlat| -> TransitionGroundingIdFlat {
                let tr_gr_idflat = tr_gr.idflat();

                self.presences_transition_ground_decomp
                    .entry(tr_gr.transition_id)
                    .or_default()
                    .insert(tr_gr_idflat);

                self.presences_transition_ground_decomp_sources
                    .entry((tr_gr.transition_id, tr_gr_idflat))
                    .or_default()
                    .insert(src_gr_idflat);

                tr_gr_idflat
            };
        let mut process_causal_link_grounding =
            |tr1: &Transition, tr1_gr: &TransitionGrounding, tr2: &Transition, tr2_gr: &TransitionGrounding| {
                if !is_causal_link_grounding_valid(tr1, tr1_gr, tr2, tr2_gr) {
                    return;
                }
                let (tr1_id, tr1_gr_idflat) = (tr1_gr.transition_id, tr1_gr.idflat());
                let (tr2_id, tr2_gr_idflat) = (tr2_gr.transition_id, tr2_gr.idflat());

                self.causal_links_ground_decomp
                    .entry((tr1_id, tr2_id))
                    .or_default()
                    .insert((tr1_gr_idflat, tr2_gr_idflat));

                self.causal_links_inflow_ground
                    .entry((tr2_id, tr2_gr_idflat))
                    .or_default()
                    .entry(tr1_id)
                    .or_default()
                    .insert(tr1_gr_idflat);

                if matches!(tr2, Transition::CondEff(_, _)) {
                    self.causal_links_outflow_pure_ground
                        .entry((tr1_id, tr1_gr_idflat))
                        .or_default()
                        .entry(tr2_id)
                        .or_default()
                        .insert(tr2_gr_idflat);
                }
            };

        let src_ids = [None].into_iter().chain(
            ctx.main
                .sched
                .tasks
                .iter()
                .enumerate()
                .map(|(task_id, _task)| Some(TaskId::from_int(u32::try_from(task_id).unwrap()))),
        );
        for src_id in src_ids {
            let Some(tr_ids) = ctx.transitions.get_for_source(&src_id) else {
                continue;
            };

            for (tr_id, _) in tr_ids {
                self.presences.push((src_id, tr_id));

                let mut src_grs = ctx.iter_source_groundings(src_id);
                while let Some(src_gr) = src_grs.next() {
                    let src_gr_idflat = process_source_grounding(src_gr);

                    for term_gr in src_gr.to_term_groundings(ctx) {
                        self.terms_ground_decomp_sources
                            .entry((term_gr.term, term_gr.assignment_id))
                            .or_default()
                            .entry(src_id)
                            .or_default()
                            .insert(src_gr_idflat);

                        self.terms_ground_decomp_leq1
                            .entry(term_gr.term)
                            .or_default()
                            .insert(term_gr.assignment_id);
                    }

                    let Some(trs_grs) = src_gr.get_transitions_groundings(ctx) else {
                        continue;
                    };
                    for tr_gr in trs_grs {
                        let tr_gr_idflat = process_transition_grounding(&tr_gr, src_gr_idflat);

                        for term_gr in tr_gr.to_term_groundings(ctx) {
                            self.terms_ground_decomp_transitions
                                .entry((term_gr.term, term_gr.assignment_id))
                                .or_default()
                                .entry(tr_gr.transition_id)
                                .or_default()
                                .insert(tr_gr_idflat);

                            self.terms_ground_decomp_leq1
                                .entry(term_gr.term)
                                .or_default()
                                .insert(term_gr.assignment_id);
                        }
                    }
                }
            }
        }

        for ((tr2_id, tr2), src2_id) in ctx.transitions.iter() {
            let tr2_is_init = src2_id.is_none() && matches!(tr2, Transition::Eff(_));
            if tr2_is_init {
                continue;
            }
            let tr1s = collect_candidate_supporters(&tr2, ctx);

            for &(tr1_id, &tr1) in &tr1s {
                debug_assert!(tr1_id != tr2_id && !matches!(tr1, Transition::Cond(_)));
                let src1_id = tr1.get_source_id(&ctx.main);

                let tr1_is_final = src1_id.is_none() && matches!(tr2, Transition::Cond(_));
                if tr1_is_final {
                    continue;
                }

                self.causal_links.push((tr1_id, tr2_id));
                self.causal_links_inflow.entry(tr2_id).or_default().insert(tr1_id);
                if matches!(tr2, Transition::CondEff(_, _)) {
                    self.causal_links_outflow_pure.entry(tr1_id).or_default().insert(tr2_id);
                }

                let mut src1_grs = ctx.iter_source_groundings(src1_id);
                while let Some(src1_gr) = src1_grs.next() {
                    let tr1_gr = src1_gr.get_transition_grounding(tr1_id, ctx).unwrap();

                    let mut src2_grs = ctx.iter_source_groundings(src2_id);
                    while let Some(src2_gr) = src2_grs.next() {
                        let tr2_gr = src2_gr.get_transition_grounding(tr2_id, ctx).unwrap();

                        process_causal_link_grounding(&tr1, &tr1_gr, &tr2, &tr2_gr);
                    }
                }
            }
        }
    }

    fn populate_cols_and_rows(&mut self, ctx: &mut SchedEncoderExt) {
        let mut _lprelax = ctx.lprelax.as_mut().unwrap();

        for &(src_id, tr_id) in &self.presences {
            self.rows.push((
                None,
                vec![
                    (ColTag::PresenceTransition(tr_id), 1.),
                    (ColTag::PresenceSource(src_id), -1.),
                ],
                Some(0.),
            ));

            self.col_tags
                .entry(ColTag::PresenceSource(src_id))
                .or_insert_with(|| _lprelax.add_column_01());

            assert!(
                self.col_tags
                    .insert(ColTag::PresenceTransition(tr_id), _lprelax.add_column_01())
                    .is_none()
            );
        }

        for (&(tr_id, tr_gr), src_gr_idflats) in &self.presences_transition_ground_decomp_sources {
            let src_id = ctx.transitions.get(tr_id).unwrap().get_source_id(&ctx.main);

            let row = (
                Some(0.),
                src_gr_idflats
                    .iter()
                    .map(|&src_gr_idflat| (ColTag::PresenceSourceGround(src_id, src_gr_idflat), 1.))
                    .chain([(ColTag::PresenceTransitionGround(tr_id, tr_gr), -1.)])
                    .collect(),
                Some(0.),
            );
            self.rows.push(row);

            for &src_gr in src_gr_idflats {
                self.col_tags
                    .entry(ColTag::PresenceSourceGround(src_id, src_gr))
                    .or_insert_with(|| _lprelax.add_column_01());
            }
        }

        for (&tr_id, tr_gr_idflats) in &self.presences_transition_ground_decomp {
            let row = (
                Some(0.),
                tr_gr_idflats
                    .iter()
                    .map(|&tr_gr_idflat| (ColTag::PresenceTransitionGround(tr_id, tr_gr_idflat), 1.))
                    .chain([(ColTag::PresenceTransition(tr_id), -1.)])
                    .collect(),
                Some(0.),
            );
            self.rows.push(row);

            for &tr_gr in tr_gr_idflats {
                self.col_tags
                    .entry(ColTag::PresenceTransitionGround(tr_id, tr_gr))
                    .or_insert_with(|| _lprelax.add_column_01());
            }
        }

        for (&term, term_gr_ids) in &self.terms_ground_decomp_leq1 {
            let row = (
                None,
                term_gr_ids
                    .iter()
                    .map(|&term_gr_id| (ColTag::TermGround(term, term_gr_id), 1.))
                    .collect(),
                Some(1.),
            );
            self.rows.push(row);

            for &term_gr_id in term_gr_ids {
                self.col_tags
                    .entry(ColTag::TermGround(term, term_gr_id))
                    .or_insert_with(|| _lprelax.add_column_01());
            }
        }

        for (&(term, term_gr_id), src_grs) in &self.terms_ground_decomp_sources {
            for (&src_id, src_gr_ids) in src_grs {
                let row = (
                    Some(0.),
                    src_gr_ids
                        .iter()
                        .map(|&src_gr_id| (ColTag::PresenceSourceGround(src_id, src_gr_id), 1.))
                        .chain([(ColTag::TermGround(term, term_gr_id), -1.)])
                        .collect(),
                    Some(0.),
                );
                self.rows.push(row);                
            }
        }

        for (&(term, term_gr_id), tr_grs) in &self.terms_ground_decomp_transitions {
            for (&tr_id, tr_gr_ids) in tr_grs {
                let row = (
                    Some(0.),
                    tr_gr_ids
                        .iter()
                        .map(|&tr_gr_id| (ColTag::PresenceTransitionGround(tr_id, tr_gr_id), 1.))
                        .chain([(ColTag::TermGround(term, term_gr_id), -1.)])
                        .collect(),
                    Some(0.),
                );
                self.rows.push(row);                
            }
        }

        for &(tr1_id, tr2_id) in &self.causal_links {
            let row1 = (
                None,
                vec![
                    (ColTag::CausalLink(tr1_id, tr2_id), 1.),
                    (ColTag::PresenceTransition(tr1_id), -1.),
                ],
                Some(0.),
            );
            let row2 = (
                None,
                vec![
                    (ColTag::CausalLink(tr1_id, tr2_id), 1.),
                    (ColTag::PresenceTransition(tr2_id), -1.),
                ],
                Some(0.),
            );
            self.rows.push(row1);
            self.rows.push(row2);

            self.col_tags
                .entry(ColTag::CausalLink(tr1_id, tr2_id))
                .or_insert_with(|| _lprelax.add_column_01());
        }

        for &(tr1_id, tr2_id) in &self.causal_links {
            if self.col_tags.contains_key(&ColTag::CausalLink(tr1_id, tr2_id)) && self.col_tags.contains_key(&ColTag::CausalLink(tr2_id, tr1_id)){
                let row = (
                    None,
                    vec![
                        (ColTag::CausalLink(tr1_id, tr2_id), 1.),
                        (ColTag::CausalLink(tr2_id, tr1_id), 1.),
                    ],
                    Some(1.),
                );
                self.rows.push(row);
            }
        }

        for (&(tr1_id, tr2_id), trs_grs_idflats) in &self.causal_links_ground_decomp {
            for &(tr1_gr_idflat, tr2_gr_idflat) in trs_grs_idflats {
                let row = (
                    None,
                    vec![
                        (
                            ColTag::CausalLinkGround(tr1_id, tr2_id, tr1_gr_idflat, tr2_gr_idflat),
                            1.,
                        ),
                        (ColTag::PresenceTransitionGround(tr1_id, tr1_gr_idflat), -1.),
                    ],
                    Some(0.),
                );
                self.rows.push(row);

                self.col_tags
                    .entry(ColTag::CausalLinkGround(tr1_id, tr2_id, tr1_gr_idflat, tr2_gr_idflat))
                    .or_insert_with(|| _lprelax.add_column_01());
            }
            let row = (
                Some(0.),
                trs_grs_idflats
                    .iter()
                    .map(|&(tr1_gr_idflat, tr2_gr_idflat)| {
                        (
                            ColTag::CausalLinkGround(tr1_id, tr2_id, tr1_gr_idflat, tr2_gr_idflat),
                            1.,
                        )
                    })
                    .chain([(ColTag::CausalLink(tr1_id, tr2_id), -1.)])
                    .collect(),
                Some(0.),
            );
            self.rows.push(row);
        }

        for (&tr2_id, tr1_ids) in &self.causal_links_inflow {
            let row = (
                Some(0.),
                tr1_ids
                    .iter()
                    .map(|&tr1_id| (ColTag::CausalLink(tr1_id, tr2_id), 1.))
                    .chain([(ColTag::PresenceTransition(tr2_id), -1.)])
                    .collect(),
                Some(0.),
            );
            self.rows.push(row);
        }

        for (&(tr2_id, tr2_gr_idflat), tr1s) in &self.causal_links_inflow_ground {
            let row = (
                Some(0.),
                tr1s.iter()
                    .flat_map(|(tr1_id, tr1_grs_idflats)| {
                        tr1_grs_idflats.iter().map(|tr1_gr_idflat| {
                            (
                                ColTag::CausalLinkGround(*tr1_id, tr2_id, *tr1_gr_idflat, tr2_gr_idflat),
                                1.,
                            )
                        })
                    })
                    .chain([(ColTag::PresenceTransitionGround(tr2_id, tr2_gr_idflat), -1.)])
                    .collect(),
                Some(0.),
            );
            self.rows.push(row);
        }

        for (&tr1_id, tr2_ids) in &self.causal_links_outflow_pure {
            let row = (
                None,
                tr2_ids
                    .iter()
                    .map(|&tr2_id| (ColTag::CausalLink(tr1_id, tr2_id), 1.))
                    .chain([(ColTag::PresenceTransition(tr1_id), -1.)])
                    .collect(),
                Some(0.),
            );
            self.rows.push(row);
        }

        for (&(tr1_id, tr1_gr_idflat), tr2s) in &self.causal_links_outflow_pure_ground {
            let row = (
                None,
                tr2s.iter()
                    .flat_map(|(tr2_id, tr2_grs)| {
                        tr2_grs.iter().map(|tr2_gr_idflat| {
                            (
                                ColTag::CausalLinkGround(tr1_id, *tr2_id, tr1_gr_idflat, *tr2_gr_idflat),
                                1.,
                            )
                        })
                    })
                    .chain([(ColTag::PresenceTransitionGround(tr1_id, tr1_gr_idflat), -1.)])
                    .collect(),
                Some(0.),
            );
            self.rows.push(row);
        }

        for (lb, row_coefs, ub) in &self.rows {
            let row_coefs = row_coefs
                .iter()
                .map(|(col_tag, coef)| (*self.col_tags.get(col_tag).unwrap(), *coef));
            _lprelax.add_row(row_coefs, *lb, *ub);
        }
    }

    fn register_lprelax_reasoner_impliers(&mut self, ctx: &mut SchedEncoderExt) {
        let bounds = BTreeMap::from_iter(
            self.terms_ground_decomp_leq1
                .keys()
                .map(|term| (term.variable(), ctx.bounds(term.variable()))),
        );

        let lprelax = ctx.lprelax.as_mut().unwrap();

        let get_term_value_from_id = |var: VarRef, term_gr_id: TermGroundingId| -> Option<IntCst> {
            //ctx.get_term_value_from_id(&IntTerm::from(var), term_gr_id.0).unwrap()
            let term = IntTerm::from(var);
            let (lb, ub) = bounds.get(&term.variable()).unwrap();
            (term_gr_id.0 < usize::try_from(ub - lb + 1).unwrap())
                .then(|| term.cst() + lb + term.factor() * IntCst::try_from(term_gr_id.0).unwrap())
        };

        let mut presence_lits_and_cols = BTreeMap::<Lit, BTreeSet<usize>>::new();
        for &(src_id, tr_id) in &self.presences {
            let p = if let Some(src_id) = src_id {
                ctx.main.sched.tasks[src_id].presence
            } else {
                Lit::TRUE
            };
            let col = *self.col_tags.get(&ColTag::PresenceSource(src_id)).unwrap();
            presence_lits_and_cols.entry(p).or_default().insert(col.index());

            let p = ctx.transitions.get(tr_id).unwrap().get_prez(&ctx.main);
            let col = *self.col_tags.get(&ColTag::PresenceTransition(tr_id)).unwrap();
            presence_lits_and_cols.entry(p).or_default().insert(col.index());
        }
        for (p, cols) in presence_lits_and_cols {
            if p.tautological() {
                for col in &cols {
                    lprelax.add_row([(LpCol::from(*col), 1.)], Some(1.), None);
                }
            } else if p.absurd() {
                for col in &cols {
                    lprelax.add_row([(LpCol::from(*col), 1.)], None, Some(1.));
                }
            } else {
                let p = p.variable();
                assert!(p != VarRef::ZERO);

                for col in &cols {
                    let col = LpCol::from(*col);

                    lprelax.register_lplit_implier(col, new_default_lplit_implier(p, col));
                }
                lprelax.register_lit_implier(
                    p,
                    std::sync::Arc::new(move |lit: Lit| {
                        assert_eq!(lit.variable(), p);
                        Some(
                            cols.iter()
                                .map(|&col| LpLit::from_model_lit(LpCol::from(col), lit))
                                .collect(),
                        )
                    }),
                );
            }
        }

        for (&term, term_gr_ids) in &self.terms_ground_decomp_leq1 {
            if term.is_cst() {
                continue;
            }
            let var = term.variable();
            assert!(var != VarRef::ZERO);

            let mappings: Vec<(LpCol, IntCst)> = term_gr_ids
                .iter()
                .map(|&term_gr_id| {
                    let val = get_term_value_from_id(var, term_gr_id).unwrap();
                    let col = *self.col_tags.get(&ColTag::TermGround(term, term_gr_id)).unwrap();
                    (col, val)
                })
                .collect();

            lprelax.register_lit_implier(
                var,
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

            for &term_gr_id in term_gr_ids {
                //let val = ctx
                //    .get_term_value_from_id(&IntTerm::from(term.variable()), term_gr.0)
                //    .unwrap();
                let val = get_term_value_from_id(term.variable(), term_gr_id).unwrap();
                let col = *self.col_tags.get(&ColTag::TermGround(term, term_gr_id)).unwrap();

                lprelax.register_lplit_implier(
                    col,
                    std::sync::Arc::new(move |lplit: LpLit| {
                        assert_eq!(lplit.col, col);
                        if lplit.tpe == LpLitType::LB && lplit.val == 1 {
                            Some(smallvec::smallvec![var.geq(val), var.leq(val)])
                        } else {
                            None
                        }
                    }),
                );
            }
        }

        for &(tr1_id, tr2_id) in &self.causal_links {
            let col = *self.col_tags.get(&ColTag::CausalLink(tr1_id, tr2_id)).unwrap();
            if let Some(s) = match (
                ctx.transitions.get(tr1_id).unwrap(),
                ctx.transitions.get(tr2_id).unwrap(),
            ) {
                (Transition::Eff(eid), Transition::Cond(cid)) => {
                    Some(ctx.main.causal_links.store[cid].get(eid).unwrap().variable())
                }
                (Transition::Eff(eid), Transition::CondEff(cid, _)) => {
                    Some(ctx.main.causal_links.store[cid].get(eid).unwrap().variable())
                }
                (Transition::CondEff(_, eid), Transition::Cond(cid)) => {
                    Some(ctx.main.causal_links.store[cid].get(eid).unwrap().variable())
                }
                (Transition::CondEff(_, eid), Transition::CondEff(cid, _)) => {
                    Some(ctx.main.causal_links.store[cid].get(eid).unwrap().variable())
                }
                (Transition::Eff(_), Transition::Eff(_)) => None,
                (Transition::CondEff(_, _), Transition::Eff(_)) => None,
                _ => unreachable!(),
            } {
                debug_assert!(s.variable() != VarRef::ZERO);

                lprelax.register_lit_implier(s, new_default_lit_implier(s, col));
                lprelax.register_lplit_implier(col, new_default_lplit_implier(s, col));
            }
        }
    }
}

impl BoolExpr<SchedEncoderExt> for LpRelaxEncoding {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoderExt) {
        assert!(l.tautological());
        // TODO: assert this being the last constraint encoded ?
        // (equivalently, all other constraints already having been encoded ?)

        assert!(ctx.lprelax.replace(LpRelax::default()).is_none());

        let mut enc = LpRelaxEncodingData::default();
        enc.populate_tags(ctx);
        enc.populate_cols_and_rows(ctx);
        enc.register_lprelax_reasoner_impliers(ctx);
    }

    fn conj_scope(&self, _ctx: &SchedEncoderExt) -> Conjunction {
        Conjunction::tautology()
    }
}
