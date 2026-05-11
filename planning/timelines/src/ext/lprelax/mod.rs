use std::collections::{BTreeMap, BTreeSet};

use aries::core::VarRef;
use aries::core::views::Term;
use itertools::Itertools;

use aries::model::lang::BoolExpr;
use aries::prelude::{Conjunction, IntCst, Lit};
use aries::utils::StreamingIterator;

use aries_lprelax::*;

use crate::IntTerm;
use crate::ext::SchedEncoderExt;
use crate::ext::encoder::Source;
use crate::ext::ground::{
    SourceTermsGround, SourceTermsGroundId, TermGround, TermGroundId, TransitionTermsGround, TransitionTermsGroundId,
};
use crate::ext::transition::TransitionId;

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
enum ColTag {
    PresenceSource(Source),
    PresenceSourceGround(Source, SourceTermsGroundId),
    PresenceTransition(TransitionId),
    PresenceTransitionGround(TransitionId, TransitionTermsGroundId),
    Support(TransitionId, TransitionId),
    SupportGround(
        TransitionId,
        TransitionId,
        TransitionTermsGroundId,
        TransitionTermsGroundId,
    ),
    TermGround(IntTerm, TermGroundId),
}

#[derive(Debug, Default)]
struct Presences {
    /// Holds (lifted) sources and their (lifted) associated transitions,
    /// as well as an "active" literal of the main CSP model indicating whether the source is active (present).
    lifted_transitions_and_sources: Vec<(Source, TransitionId, Lit)>,

    /// Holds (IDs of) sources' groundings.
    ground_sources: BTreeMap<Source, BTreeSet<SourceTermsGroundId>>,
    /// Holds (IDs of) transitions' groundings, as well as (IDs of) compatible (i.e. superset) groundings of their sources.
    ground_transitions: BTreeMap<TransitionId, BTreeMap<TransitionTermsGroundId, BTreeSet<SourceTermsGroundId>>>,

    /// Holds (IDs of) groundings of terms appearing in transitions and/or sources.
    ground_terms: BTreeMap<IntTerm, BTreeSet<TermGroundId>>,
    /// Holds (IDs of) groundings of terms, as well as (IDs of) groundings of sources in which they appear.
    ground_terms_sources: BTreeMap<(IntTerm, TermGroundId), BTreeMap<Source, BTreeSet<SourceTermsGroundId>>>,
    /// Holds (IDs of) groundings of terms, as well as (IDs of) groundings of transitions in which they appear.
    ground_terms_transitions:
        BTreeMap<(IntTerm, TermGroundId), BTreeMap<TransitionId, BTreeSet<TransitionTermsGroundId>>>,
}
impl Presences {
    /// Registers a (lifted) transition and its (lifted) source, as well as an "active" literal of
    /// the main CSP model indicating whether the source is active (present).
    pub fn add_lifted_transition_and_source(&mut self, src: Source, tr_id: TransitionId, src_active: Lit) {
        self.lifted_transitions_and_sources.push((src, tr_id, src_active));
    }
    /// Registers a grounding of a source.
    pub fn add_ground_source(&mut self, src_grounding: &SourceTermsGround, ctx: &SchedEncoderExt) {
        self.ground_sources
            .entry(src_grounding.source)
            .or_default()
            .insert(src_grounding.get_id());

        for term_grounding in src_grounding.to_term_groundings(ctx) {
            if term_grounding.term.is_cst() {
                continue;
            }
            self.ground_terms
                .entry(term_grounding.term)
                .or_default()
                .insert(term_grounding.id);

            self.ground_terms_sources
                .entry((term_grounding.term, term_grounding.id))
                .or_default()
                .entry(src_grounding.source)
                .or_default()
                .insert(src_grounding.get_id());
        }
    }
    /// Registers a grounding of a transition, and the given grounding of its source as compatible with it.
    pub fn add_ground_transition(
        &mut self,
        tr_grounding: &TransitionTermsGround,
        src_grounding: &SourceTermsGround,
        ctx: &SchedEncoderExt,
    ) {
        self.ground_transitions
            .entry(tr_grounding.transition_id)
            .or_default()
            .entry(tr_grounding.get_id())
            .or_default()
            .insert(src_grounding.get_id());

        for term_grounding in tr_grounding.to_term_groundings(ctx) {
            if term_grounding.term.is_cst() {
                continue;
            }
            self.ground_terms
                .entry(term_grounding.term)
                .or_default()
                .insert(term_grounding.id);

            self.ground_terms_transitions
                .entry((term_grounding.term, term_grounding.id))
                .or_default()
                .entry(tr_grounding.transition_id)
                .or_default()
                .insert(tr_grounding.get_id());
        }
    }
}
#[derive(Debug, Default)]
struct Supports {
    /// Holds (potential) supports between transitions, as well as an "active" literal of
    /// the main CSP model indicating whether the support is active.
    ///
    /// Note that here, effect transitions are allowed to
    /// be supporters of other effect transitions (on the same predicate / state function),
    /// which is not the case in our main definition of causal links.
    ///
    /// In this specific case where the support is between two effects,
    /// the "active" literal is None (as this doesn't correspond to a causal link in the main CSP model).
    lifted: BTreeMap<(TransitionId, TransitionId), Option<Lit>>,
    /// For a given transition, holds its "incoming flow",
    /// i.e. the set of transitions that can support it.
    lifted_inflow: BTreeMap<TransitionId, BTreeSet<TransitionId>>,
    /// For a given transition, holds its "pure" "outgoing flow",
    /// i.e. the set of Eff or CondEff transitions that it can support.
    lifted_pure_outflow: BTreeMap<TransitionId, BTreeSet<TransitionId>>,

    /// Holds the (IDs of) groundings of a support.
    ground: BTreeMap<(TransitionId, TransitionId), BTreeSet<(TransitionTermsGroundId, TransitionTermsGroundId)>>,
    /// For a given ground transition, holds the (IDs of) groundings of transitions that can support it ("incoming flow").
    ground_inflow:
        BTreeMap<(TransitionId, TransitionTermsGroundId), BTreeMap<TransitionId, BTreeSet<TransitionTermsGroundId>>>,
    /// For a given ground transition, holds the (IDs of) groundings of Eff or CondEff transitions that it can support ("pure" "outgoing flow").
    ground_pure_outflow:
        BTreeMap<(TransitionId, TransitionTermsGroundId), BTreeMap<TransitionId, BTreeSet<TransitionTermsGroundId>>>,
}
impl Supports {
    /// Registers a (lifted) support relation between two (appropriate) (lifted) transitions.
    pub fn add_lifted(&mut self, tr1_id: TransitionId, tr2_id: TransitionId, active: Option<Lit>) {
        debug_assert!(!matches!(tr1_id, TransitionId::Cond(_)));

        self.lifted.insert((tr1_id, tr2_id), active);

        self.lifted_inflow.entry(tr2_id).or_default().insert(tr1_id);

        if matches!(tr2_id, TransitionId::Eff(_) | TransitionId::CondEff(_, _)) {
            self.lifted_pure_outflow.entry(tr1_id).or_default().insert(tr2_id);
        }
    }
    /// Registers a grounding of support relation.
    pub fn add_ground(&mut self, tr1_grounding: &TransitionTermsGround, tr2_grounding: &TransitionTermsGround) {
        debug_assert!(!matches!(tr1_grounding.transition_id, TransitionId::Cond(_)));

        self.ground
            .entry((tr1_grounding.transition_id, tr2_grounding.transition_id))
            .or_default()
            .insert((tr1_grounding.get_id(), tr2_grounding.get_id()));

        self.ground_inflow
            .entry((tr2_grounding.transition_id, tr2_grounding.get_id()))
            .or_default()
            .entry(tr1_grounding.transition_id)
            .or_default()
            .insert(tr1_grounding.get_id());

        if matches!(
            tr2_grounding.transition_id,
            TransitionId::Eff(_) | TransitionId::CondEff(_, _)
        ) {
            self.ground_pure_outflow
                .entry((tr1_grounding.transition_id, tr1_grounding.get_id()))
                .or_default()
                .entry(tr2_grounding.transition_id)
                .or_default()
                .insert(tr2_grounding.get_id());
        }
    }
    /// Returns if a support grounding is valid (used to determine if it should be registered).
    /// This means the same ground state variable, and additionally:
    pub fn ground_is_valid(
        &self,
        tr1_grounding: &TransitionTermsGround,
        tr2_grounding: &TransitionTermsGround,
    ) -> bool {
        let n = tr1_grounding.transition_ref.get_args().len();
        debug_assert!(n == tr2_grounding.transition_ref.get_args().len());

        let compatible_ground_args = tr1_grounding.get_assignment()[..n] == tr2_grounding.get_assignment()[..n];

        let compatible_ground_values = match (
            tr1_grounding.transition_ref.get_valto_idx(),
            tr2_grounding.transition_ref.get_valfrom_idx(),
        ) {
            (Some(i), Some(j)) => tr1_grounding.get_assignment()[i] == tr2_grounding.get_assignment()[j],
            (Some(_), None) => true,
            (None, _) => unreachable!(),
        };

        compatible_ground_args && compatible_ground_values
    }
}

#[derive(Debug)]
enum TagsExpr {
    Eq(Vec<ColTag>, Vec<ColTag>),
    Leq(Vec<ColTag>, Vec<ColTag>),
    Leq1(Vec<ColTag>),
}
#[derive(Debug, Default)]
struct LpRelaxEncodingData {
    presences: Presences,
    supports: Supports,

    col_tags: BTreeMap<ColTag, LpCol>,
    tags_exprs: Vec<TagsExpr>,
}

fn iter_supports(ctx: &SchedEncoderExt) -> impl Iterator<Item = ((TransitionId, TransitionId), Option<Lit>)> {
    let supports_from_ctx_encoded_causal_links = ctx.main.causal_links.get_links().map(|cl| {
        let tr1_id = ctx.get_transition_of_effect(cl.eff_id).unwrap();
        let tr2_id = ctx.get_transition_of_condition(cl.cond_id).unwrap();
        debug_assert!(
            ctx.get_transition(tr1_id).get_state_var().fluent == ctx.get_transition(tr2_id).get_state_var().fluent
        );
        ((tr1_id, tr2_id), Some(cl.active))
    });
    let known_effects_iter = || ctx.iter_nondefault_effects();
    let default_initial_effects_iter = || ctx.iter_default_effects();

    let supports_from_known_effects_to_others = known_effects_iter().flat_map(move |(eff1_id, _)| {
        let tr1_id = ctx.get_transition_of_effect(eff1_id).unwrap();
        #[allow(clippy::let_and_return)]
        let to_other_known_effects = known_effects_iter().flat_map(move |(eff2_id, _)| {
            let tr2_id = ctx.get_transition_of_effect(eff2_id).unwrap();
            if tr1_id == tr2_id {
                return None;
            }
            if matches!(tr2_id, TransitionId::CondEff(_, _)) {
                return None;
            }
            debug_assert!(matches!(tr2_id, TransitionId::Eff(_)));

            let (tr1, tr2) = (ctx.get_transition(tr1_id), ctx.get_transition(tr2_id));
            tr2.get_source()?;

            if tr1.get_state_var().fluent == tr2.get_state_var().fluent {
                if tr1
                    .get_args()
                    .iter()
                    .zip(tr2.get_args().iter())
                    .any(|(t1, t2)| t1.is_cst() && t2.is_cst() && t1.cst() != t2.cst())
                {
                    return None;
                }
                Some(((tr1_id, tr2_id), None))
            } else {
                None
            }
        });
        to_other_known_effects
    });
    let supports_from_default_initial_effects = default_initial_effects_iter().flat_map(move |(eff1_id, _)| {
        let tr1_id = ctx.get_transition_of_effect(eff1_id).unwrap();
        debug_assert!(ctx.get_transition(tr1_id).iter_terms().all(|term| term.is_cst()));
        let to_conds = ctx.iter_conditions().filter_map(move |(cond_id, _)| {
            let tr2_id = ctx.get_transition_of_condition(cond_id).unwrap();
            debug_assert!(tr1_id != tr2_id);

            if ctx.get_transition(tr1_id).get_state_var().fluent == ctx.get_transition(tr2_id).get_state_var().fluent {
                if ctx
                    .get_transition(tr1_id)
                    .get_args()
                    .iter()
                    .zip(ctx.get_transition(tr2_id).get_args().iter())
                    .any(|(t1, t2)| t1.is_cst() && t2.is_cst() && t1.cst() != t2.cst())
                {
                    return None;
                }
                if ctx.get_transition(tr1_id).get_valto().unwrap().is_cst()
                    && ctx
                        .get_transition(tr2_id)
                        .get_valfrom()
                        .map(|t| t.is_cst() && ctx.get_transition(tr1_id).get_valto().unwrap().cst() != t.cst())
                        .unwrap_or_default()
                {
                    return None;
                }
                Some(((tr1_id, tr2_id), None))
            } else {
                None
            }
        });
        let to_known_effects = known_effects_iter().flat_map(move |(eff2_id, _)| {
            let tr2_id = ctx.get_transition_of_effect(eff2_id).unwrap();
            if tr1_id == tr2_id {
                return None;
            }
            if matches!(tr2_id, TransitionId::CondEff(_, _)) {
                return None;
            }
            debug_assert!(matches!(tr2_id, TransitionId::Eff(_)));

            ctx.get_transition(tr2_id).get_source()?;

            if ctx.get_transition(tr1_id).get_state_var().fluent == ctx.get_transition(tr2_id).get_state_var().fluent {
                if ctx
                    .get_transition(tr1_id)
                    .get_args()
                    .iter()
                    .zip(ctx.get_transition(tr2_id).get_args().iter())
                    .any(|(t1, t2)| t1.is_cst() && t2.is_cst() && t1.cst() != t2.cst())
                {
                    return None;
                }
                Some(((tr1_id, tr2_id), None))
            } else {
                None
            }
        });
        std::iter::chain(to_conds, to_known_effects)
    });

    supports_from_ctx_encoded_causal_links
        .chain(supports_from_known_effects_to_others)
        .chain(supports_from_default_initial_effects)
        .inspect(|&((tr1_id, tr2_id), _)| {
            debug_assert!(
                tr1_id != tr2_id,
                "{:?} --- {:?}",
                ctx.get_transition(tr1_id),
                ctx.get_transition(tr2_id)
            );
            debug_assert!(!matches!(tr1_id, TransitionId::Cond(_)));
            debug_assert!(!matches!(tr2_id, TransitionId::Eff(_)) || ctx.get_transition(tr2_id).get_source().is_some());
        })
}

impl LpRelaxEncodingData {
    fn collect_relations(&mut self, ctx: &mut SchedEncoderExt) {
        // Collect lifted presences of transitions and sources,
        // as well as the relations between groundings of transitions, sources, and terms appearing in them.
        for src in ctx.iter_sources() {
            for tr_id in ctx.get_transitions_of_source(&src) {
                let src_active = ctx.get_source(&src).map(|task| task.presence).unwrap_or(Lit::TRUE);
                self.presences.add_lifted_transition_and_source(src, tr_id, src_active);

                let mut src_groundings_iter = ctx.iter_source_groundings(src);
                while let Some(src_grounding) = src_groundings_iter.next() {
                    self.presences.add_ground_source(src_grounding, ctx);

                    for tr_grounding in src_grounding.to_transitions_groundings(ctx) {
                        self.presences.add_ground_transition(&tr_grounding, src_grounding, ctx);
                    }
                }
            }
        }
        // Collect lifted and ground supports between transitions.
        // Note that here (in the LP relaxation), effect transitions are allowed to
        // be supporters of other effect transitions (on the same predicate / state function),
        // which is not the case in our main definition of causal links.
        // In this specific case where the support is between two effects,
        // the "active" literal is None (as this doesn't correspond to a causal link in the main CSP model).
        for ((tr1_id, tr2_id), active) in iter_supports(ctx) {
            self.supports.add_lifted(tr1_id, tr2_id, active);

            let mut src1_groundings_iter = ctx.iter_source_groundings(ctx.get_transition(tr1_id).get_source());
            while let Some(src1_gr) = src1_groundings_iter.next() {
                let tr1_grounding = src1_gr.to_transition_grounding(tr1_id, ctx);

                let mut src2_groundings_iter = ctx.iter_source_groundings(ctx.get_transition(tr2_id).get_source());
                while let Some(src2_gr) = src2_groundings_iter.next() {
                    let tr2_grounding = src2_gr.to_transition_grounding(tr2_id, ctx);

                    if self.supports.ground_is_valid(&tr1_grounding, &tr2_grounding) {
                        self.supports.add_ground(&tr1_grounding, &tr2_grounding);
                    }
                }
            }
        }
    }

    fn build_cols_and_rows(&mut self, ctx: &mut SchedEncoderExt) {
        // Instead of doing `let lprelax = ctx.lprelax.as_mut().unwrap()`,
        // temporarily take `lprelax` out of the option in ctx, to allow borrowing ctx as immutable (when using `eval`).
        // At the end of this method, put `lprelax` back into the option (`ctx.lprelax = Some(lprelax)`).
        let mut lprelax = ctx.lprelax.take().unwrap();

        let mut add_tags_expr = |tags_expr: TagsExpr| {
            self.tags_exprs.push(tags_expr);
        };
        let mut add_column_01 = |col_tag: ColTag| {
            self.col_tags.entry(col_tag).or_insert_with(|| lprelax.add_column_01());
        };

        // A transition is active (i.e. present) iff its source is
        for &(src, tr_id, _) in &self.presences.lifted_transitions_and_sources {
            add_tags_expr(TagsExpr::Eq(
                vec![ColTag::PresenceTransition(tr_id)],
                vec![ColTag::PresenceSource(src)],
            ));

            add_column_01(ColTag::PresenceSource(src));
            add_column_01(ColTag::PresenceTransition(tr_id));
        }

        // A source is active iff one of its groundings is.
        for (&src, src_groundings_ids) in &self.presences.ground_sources {
            add_tags_expr(TagsExpr::Eq(
                vec![ColTag::PresenceSource(src)],
                src_groundings_ids
                    .iter()
                    .map(|&src_grounding_id| ColTag::PresenceSourceGround(src, src_grounding_id))
                    .collect(),
            ));

            for &src_grounding_id in src_groundings_ids {
                add_column_01(ColTag::PresenceSourceGround(src, src_grounding_id));
            }
        }

        // A transition is active iff one of its groundings is
        // A ground transition is active iff one of its source's compatible groundings is.
        for (&tr_id, grs) in &self.presences.ground_transitions {
            add_tags_expr(TagsExpr::Eq(
                vec![ColTag::PresenceTransition(tr_id)],
                grs.keys()
                    .map(|&tr_grounding_id| ColTag::PresenceTransitionGround(tr_id, tr_grounding_id))
                    .collect(),
            ));

            for &tr_grounding_id in grs.keys() {
                add_column_01(ColTag::PresenceTransitionGround(tr_id, tr_grounding_id));
            }

            for (&tr_grounding_id, src_groundings_ids) in grs {
                let src = ctx.get_transition(tr_id).get_source();

                add_tags_expr(TagsExpr::Eq(
                    vec![ColTag::PresenceTransitionGround(tr_id, tr_grounding_id)],
                    src_groundings_ids
                        .iter()
                        .map(|&src_grounding_id| ColTag::PresenceSourceGround(src, src_grounding_id))
                        .collect(),
                ));

                for &src_grounding_id in src_groundings_ids {
                    add_column_01(ColTag::PresenceSourceGround(src, src_grounding_id));
                }
            }
        }

        // At most one grounding of a term can be active
        for (&term, term_groundings_ids) in &self.presences.ground_terms {
            add_tags_expr(TagsExpr::Leq1(
                term_groundings_ids
                    .iter()
                    .map(|&term_grounding_id| ColTag::TermGround(term, term_grounding_id))
                    .collect(),
            ));

            for &term_grounding_id in term_groundings_ids {
                add_column_01(ColTag::TermGround(term, term_grounding_id));
            }
        }

        // A grounding of a term is active iff a source grounding using it is active
        for (&(term, term_grounding_id), src_groundings) in &self.presences.ground_terms_sources {
            for (&src, src_groundings_ids) in src_groundings {
                add_tags_expr(TagsExpr::Eq(
                    vec![ColTag::TermGround(term, term_grounding_id)],
                    src_groundings_ids
                        .iter()
                        .map(|&src_grounding_id| ColTag::PresenceSourceGround(src, src_grounding_id))
                        .collect(),
                ));
            }
        }

        // A grounding of a term is active iff a transition grounding using it is active
        for (&(term, term_grounding_id), tr_groundings) in &self.presences.ground_terms_transitions {
            for (&tr_id, tr_groundings_ids) in tr_groundings {
                add_tags_expr(TagsExpr::Eq(
                    vec![ColTag::TermGround(term, term_grounding_id)],
                    tr_groundings_ids
                        .iter()
                        .map(|&tr_grounding_id| ColTag::PresenceTransitionGround(tr_id, tr_grounding_id))
                        .collect(),
                ));
            }
        }

        // If a support is active, then both its transitions must be active
        for &(tr1_id, tr2_id) in self.supports.lifted.keys() {
            add_tags_expr(TagsExpr::Leq(
                vec![ColTag::Support(tr1_id, tr2_id)],
                vec![ColTag::PresenceTransition(tr1_id)],
            ));
            add_tags_expr(TagsExpr::Leq(
                vec![ColTag::Support(tr1_id, tr2_id)],
                vec![ColTag::PresenceTransition(tr2_id)],
            ));

            add_column_01(ColTag::Support(tr1_id, tr2_id));
        }

        // A transition1 supporting transition2 cannot be supported by transition2
        // (i.e. forbid trivial cycles)
        // NOTE: REQUIRES all initial effects (notably initial negative / default effects).
        //       Without that, the in-flow constraint (see below) would be incorrect!
        for (&tr2_id, tr1s) in &self.supports.lifted_inflow {
            for &tr1_id in tr1s {
                if self
                    .supports
                    .lifted_inflow
                    .get(&tr1_id)
                    .is_some_and(|set| set.contains(&tr2_id))
                {
                    add_tags_expr(TagsExpr::Leq1(vec![
                        ColTag::Support(tr1_id, tr2_id),
                        ColTag::Support(tr2_id, tr1_id),
                    ]));
                }
            }
        }

        // A support is active iff one of its groundings is
        // If a ground support is active, then its terms' groundings must be active
        for (&(tr1_id, tr2_id), trs_groundings_ids) in &self.supports.ground {
            add_tags_expr(TagsExpr::Eq(
                vec![ColTag::Support(tr1_id, tr2_id)],
                trs_groundings_ids
                    .iter()
                    .map(|&(tr1_grounding_id, tr2_grounding_id)| {
                        ColTag::SupportGround(tr1_id, tr2_id, tr1_grounding_id, tr2_grounding_id)
                    })
                    .collect(),
            ));

            for &(tr1_grounding_id, tr2_grounding_id) in trs_groundings_ids {
                add_tags_expr(TagsExpr::Leq(
                    vec![ColTag::SupportGround(
                        tr1_id,
                        tr2_id,
                        tr1_grounding_id,
                        tr2_grounding_id,
                    )],
                    vec![ColTag::PresenceTransitionGround(tr1_id, tr1_grounding_id)],
                ));
                add_tags_expr(TagsExpr::Leq(
                    vec![ColTag::SupportGround(
                        tr1_id,
                        tr2_id,
                        tr1_grounding_id,
                        tr2_grounding_id,
                    )],
                    vec![ColTag::PresenceTransitionGround(tr2_id, tr2_grounding_id)],
                ));

                add_column_01(ColTag::SupportGround(
                    tr1_id,
                    tr2_id,
                    tr1_grounding_id,
                    tr2_grounding_id,
                ));
            }
        }

        // A transition is present iff it is supported by another one
        // (Note that effect transitions are allowed to be supported too,
        // by transitions on the same state fluent and any value)
        for (&tr2_id, tr1_ids) in &self.supports.lifted_inflow {
            add_tags_expr(TagsExpr::Eq(
                vec![ColTag::PresenceTransition(tr2_id)],
                tr1_ids.iter().map(|&tr1_id| ColTag::Support(tr1_id, tr2_id)).collect(),
            ));
        }

        // A ground transition is present iff it is supported by another (compatible) one.
        // Same remark on effect transitions as above.
        for (&(tr2_id, tr2_grounding_id), tr1s) in &self.supports.ground_inflow {
            add_tags_expr(TagsExpr::Eq(
                tr1s.iter()
                    .flat_map(|(&tr1_id, tr1_groundings_ids)| {
                        tr1_groundings_ids.iter().map(move |&tr1_grounding_id| {
                            ColTag::SupportGround(tr1_id, tr2_id, tr1_grounding_id, tr2_grounding_id)
                        })
                    })
                    .collect(),
                vec![ColTag::PresenceTransitionGround(tr2_id, tr2_grounding_id)],
            ));
        }

        // If a (non-condition) transition is present,
        // then it can support at most one Eff or CondEff transition.
        for (&tr1_id, tr2_ids) in &self.supports.lifted_pure_outflow {
            add_tags_expr(TagsExpr::Leq(
                tr2_ids.iter().map(|&tr2_id| ColTag::Support(tr1_id, tr2_id)).collect(),
                vec![ColTag::PresenceTransition(tr1_id)],
            ));
        }

        // If a ground (non-condition) transition is present,
        // then it can support at most one (compatible) Eff or CondEff transition.
        for (&(tr1_id, tr1_grounding_id), tr2_ids) in &self.supports.ground_pure_outflow {
            add_tags_expr(TagsExpr::Leq(
                tr2_ids
                    .iter()
                    .flat_map(|(&tr2_id, tr2_groundings_ids)| {
                        tr2_groundings_ids.iter().map(move |&tr2_grounding_id| {
                            ColTag::SupportGround(tr1_id, tr2_id, tr1_grounding_id, tr2_grounding_id)
                        })
                    })
                    .collect(),
                vec![ColTag::PresenceTransitionGround(tr1_id, tr1_grounding_id)],
            ));
        }

        // Add all rows to the LP problem.
        for tags_expr in &self.tags_exprs {
            let (row_coefs, lb, ub) = match tags_expr {
                TagsExpr::Eq(lhs, rhs) => (
                    lhs.iter()
                        .map(|col_tag| (*self.col_tags.get(col_tag).unwrap(), 1.))
                        .chain(rhs.iter().map(|col_tag| (*self.col_tags.get(col_tag).unwrap(), -1.)))
                        .collect_vec(),
                    Some(0.),
                    Some(0.),
                ),
                TagsExpr::Leq(lhs, rhs) => (
                    lhs.iter()
                        .map(|col_tag| (*self.col_tags.get(col_tag).unwrap(), 1.))
                        .chain(rhs.iter().map(|col_tag| (*self.col_tags.get(col_tag).unwrap(), -1.)))
                        .collect_vec(),
                    None,
                    Some(0.),
                ),
                TagsExpr::Leq1(lhs) => (
                    lhs.iter()
                        .map(|col_tag| (*self.col_tags.get(col_tag).unwrap(), 1.))
                        .collect_vec(),
                    None,
                    Some(1.),
                ),
            };
            lprelax.add_row(row_coefs, lb, ub);
        }

        ctx.lprelax = Some(lprelax);
    }

    fn bind_to_main_model(&mut self, ctx: &mut SchedEncoderExt) {
        // Instead of doing `let lprelax = ctx.lprelax.as_mut().unwrap()`,
        // temporarily take `lprelax` out of the option in ctx, to allow borrowing ctx as immutable (when using `eval`).
        // At the end of this method, put `lprelax` back into the option (`ctx.lprelax = Some(lprelax)`).
        let mut lprelax = ctx.lprelax.take().unwrap();

        let mut presence_lits_and_cols = BTreeMap::<Lit, BTreeSet<usize>>::new();
        for &(src, tr_id, src_active) in &self.presences.lifted_transitions_and_sources {
            presence_lits_and_cols
                .entry(src_active)
                .or_default()
                .insert(self.col_tags.get(&ColTag::PresenceSource(src)).unwrap().index());
            presence_lits_and_cols
                .entry(ctx.get_transition(tr_id).get_prez())
                .or_default()
                .insert(self.col_tags.get(&ColTag::PresenceTransition(tr_id)).unwrap().index());
        }
        // Bind lifted presence columns of the LP with corresponding literals in the main CSP.
        for (p, cols) in presence_lits_and_cols {
            if p.tautological() {
                for &col in &cols {
                    lprelax.tighten_column(LpCol::from(col), Some(1.), None);
                }
            } else if p.absurd() {
                for &col in &cols {
                    lprelax.tighten_column(LpCol::from(col), None, Some(0.));
                }
            } else {
                let p = p.variable();
                assert!(p != VarRef::ZERO);

                for &col in &cols {
                    lprelax.add_col_half_binding_default(LpCol::from(col), p);
                }
                lprelax.add_var_half_binding(
                    p,
                    std::sync::Arc::new(move |lit: Lit| {
                        assert_eq!(lit.variable(), p);
                        cols.iter()
                            .map(|&col| LpLit::from_model_lit(LpCol::from(col), lit))
                            .collect()
                    }),
                );
            }
        }

        // Bind term grounding columns of the LP with corresponding literals in the main CSP.
        for (&term, term_grounding_ids) in &self.presences.ground_terms {
            if term.is_cst() {
                continue;
            }
            let var = term.variable();
            assert!(var != VarRef::ZERO);

            let mappings: Vec<(usize, IntCst)> = {
                let mut res = vec![];
                for &term_grounding_id in term_grounding_ids {
                    let col = self
                        .col_tags
                        .get(&ColTag::TermGround(term, term_grounding_id))
                        .unwrap()
                        .index();
                    let val = TermGround::from(term, term_grounding_id, ctx).assignment(ctx);
                    res.push((col, val));
                }
                res
            };
            lprelax.add_var_half_binding(
                var,
                std::sync::Arc::new(move |lit: Lit| {
                    assert_eq!(lit.variable(), var);
                    mappings
                        .iter()
                        .filter_map(|&(col, val)| {
                            (lit.entails(var.lt(val)) || lit.entails(var.gt(val)))
                                .then_some(LpLit::leq(LpCol::from(col), 0))
                        })
                        .collect()
                }),
            );

            for &term_grounding_id in term_grounding_ids {
                let col = *self.col_tags.get(&ColTag::TermGround(term, term_grounding_id)).unwrap();
                let val = TermGround::from(term, term_grounding_id, ctx).assignment(ctx);

                lprelax.add_col_half_binding(
                    col,
                    std::sync::Arc::new(move |lplit: LpLit| {
                        assert_eq!(lplit.col, col);
                        if lplit.tpe == LpLitType::GEQ && lplit.val == 1 {
                            smallvec::smallvec![var.geq(val), var.leq(val)]
                        } else {
                            Default::default()
                        }
                    }),
                );
            }
        }

        // Bind lifted support columns of the LP with corresponding literals in the main CSP.
        for (&(tr1_id, tr2_id), &s) in &self.supports.lifted {
            if let Some(s) = s {
                let s = s.variable();
                debug_assert!(s != VarRef::ZERO);

                let col = *self.col_tags.get(&ColTag::Support(tr1_id, tr2_id)).unwrap();

                lprelax.add_var_half_binding_default(s, col);
                lprelax.add_col_half_binding_default(col, s);
            }
        }

        ctx.lprelax = Some(lprelax);
    }

    #[allow(unused)]
    fn print(&self, ctx: &SchedEncoderExt) {
        println!("## Transitions");
        for (tr_id, tr) in ctx.iter_transitions() {
            println!("{:?} ==== {:?}", tr_id, tr);
        }
        println!("## Columns");
        for (col_tag, col) in self.col_tags.iter() {
            println!("{col:?} {col_tag:?}");
        }
        println!("## Rows");
        for (i, r) in self.tags_exprs.iter().enumerate() {
            println!("Row({i}) {r:?}");
        }
    }
}

#[derive(Debug)]
pub(crate) struct LpRelaxEncoding;

impl<'a> BoolExpr<SchedEncoderExt<'a>> for LpRelaxEncoding {
    fn enforce_if(&self, l: Lit, ctx: &mut SchedEncoderExt) {
        assert!(
            l.tautological(),
            "The LP relaxation constraints are defined in the global scope."
        );
        // TODO: assert this being the last constraint encoded ?
        // (equivalently, all other constraints already having been encoded ?)

        assert!(ctx.lprelax.is_none());
        ctx.lprelax = Some(LpRelax::default());

        let mut enc = LpRelaxEncodingData::default();
        enc.collect_relations(ctx);
        enc.build_cols_and_rows(ctx);
        enc.bind_to_main_model(ctx);
        // enc.print(ctx);

        assert!(ctx.lprelax.is_some());
    }

    fn conj_scope(&self, _ctx: &SchedEncoderExt) -> Conjunction {
        Conjunction::tautology()
    }
}
