mod utils;

use utils::{binary_search_range_by, merge_dedup_into};

use std::collections::HashMap;

use aries_solver::core::IntCst;
use idmap::DirectIdMap;
use itertools::Itertools;

use crate::IntTerm;
use crate::encoder::SchedEncoder;
use crate::ext::Source;
use crate::ext::lprelax::LpRelaxEncoder;
use crate::ext::lprelax::encoding::groundings::utils::merge_join_chunks_by_key;
use crate::ext::lprelax::encoding::lifted::LpRelaxEncodingLiftedSupportsSorted;
use crate::ext::lprelax::transitions::{TransitionGroundingId, TransitionId};
use crate::ext::{SourceGrounding, SourceGroundingId};

#[derive(Clone, Default)]
pub(super) struct LpRelaxEncodingGroundingsInfo {
    pub(super) ready: bool,
    sources: SourcesGroundingsInfo,
    state_vars: StateVarsGroundingsInfo,
    transitions: TransitionsGroundingsInfo,
    terms: TermsGroundingsInfo,
    supports: SupportsGroundingsInfo,
}

impl LpRelaxEncodingGroundingsInfo {
    // Interns a grounding of a source.
    // WARNING: adding duplicate groundings (for the same source) will result in a panic (in debug mode).
    //
    // Does not do anything relating to ground supports.
    pub fn post_ground_source(
        &mut self,
        source: Source,
        source_grounding: SourceGrounding,
        encoder: &LpRelaxEncoder,
        ctx: &SchedEncoder,
    ) {
        self.ready = false;

        // Will panic (in debug mode) if the source and the grounding have already been interned.
        let source_grounding_id = self.sources.post_ground_source(source, source_grounding.clone());

        // Intern each of the variable assignments in the source grounding,
        // marking each of them as appearing in it.
        {
            let source_terms = encoder.get_source_terms(source, ctx);

            for (i, &term) in source_terms.iter().enumerate() {
                let value = source_grounding[i];
                if !term.is_cst() {
                    self.terms
                        .post_for_ground_source(term, value, source, source_grounding_id);
                }
            }
        }

        // For each transition of the source, intern the corresponding grounding,
        // marking each of them as being appearing in the source grounding.
        // Also mark each of the variable assignments as appearing in the corresponding transition groundings.
        {
            for &transition_id in encoder.transitions.of_source(source) {
                let transition_grounding =
                    encoder.get_transition_terms_terms_eval_in_ground_source(transition_id, &source_grounding, ctx);
                let transition_grounding_id = TransitionGroundingId {
                    state_var_grounding_id: self.state_vars.post_ground_state_var(
                        &encoder.transitions.get_state_var(transition_id, ctx).fluent,
                        &transition_grounding.args,
                    ),
                    valfrom: transition_grounding.valfrom,
                    valto: transition_grounding.valto,
                };

                {
                    let transition_terms = encoder.get_transition_terms(transition_id, ctx);

                    for (&term, &value) in transition_terms.args.iter().zip(&transition_grounding.args) {
                        if !term.is_cst() {
                            self.terms
                                .post_for_ground_transition(term, value, transition_id, transition_grounding_id);
                        }
                    }
                    if transition_terms.valfrom.is_some_and(|term| !term.is_cst()) {
                        self.terms.post_for_ground_transition(
                            transition_terms.valfrom.unwrap(),
                            transition_grounding.valfrom.unwrap(),
                            transition_id,
                            transition_grounding_id,
                        );
                    }
                    if transition_terms.valto.is_some_and(|term| !term.is_cst()) {
                        self.terms.post_for_ground_transition(
                            transition_terms.valto.unwrap(),
                            transition_grounding.valto.unwrap(),
                            transition_id,
                            transition_grounding_id,
                        );
                    }
                }

                self.transitions.post_ground_transition(
                    transition_id,
                    transition_grounding_id,
                    source,
                    source_grounding_id,
                );
            }
        }
    }

    pub(super) fn transitions_sort(&mut self) {
        self.transitions.sort_for_all();
    }
    pub(super) fn terms_sort(&mut self) {
        self.terms.sort_and_merge_pending_sources();
        self.terms.sort_and_merge_pending_transitions();
    }
    // #[allow(dead_code)]
    // pub fn sort_all(&mut self) {
    //     self.terms.sort_and_merge_pending_sources();
    //     self.terms.sort_and_merge_pending_transitions();
    //
    //     self.transitions.sort_for_all();
    // }
    pub(super) fn supports_build(&mut self, lifted_supports_sorted: &LpRelaxEncodingLiftedSupportsSorted) {
        self.supports = SupportsGroundingsInfo::from(lifted_supports_sorted, &self.transitions);
    }

    pub(super) fn mark_ready(&mut self) {
        self.ready = true;
    }
    // pub(super) fn mark_unready(&mut self) {
    //     self.ready = false;
    // }
    #[allow(dead_code)]
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn sources(&self) -> &SourcesGroundingsInfo {
        &self.sources
    }
    pub fn transitions(&self) -> &TransitionsGroundingsInfo {
        &self.transitions
    }
    pub fn terms(&self) -> &TermsGroundingsInfo {
        &self.terms
    }
    pub fn supports(&self) -> &SupportsGroundingsInfo {
        &self.supports
    }
}

#[derive(Clone, Default)]
pub(super) struct SourcesGroundingsInfo {
    /// First element: list of (unique) groundings of the "empty source" (i.e. groundings corresponding to the "initial/final" action)
    /// Second element: list of (unique) groundings of a "concrete source" (action/task)
    entries: (
        Vec<SourceGroundingId>,
        DirectIdMap<crate::TaskId, Vec<SourceGroundingId>>,
    ),
    /// Index: id of a source grounding
    groundings_rev: Vec<(Source, SourceGrounding)>,

    #[cfg(debug_assertions)]
    groundings: HashMap<(Source, SourceGrounding), SourceGroundingId>,
}
impl SourcesGroundingsInfo {
    /// Interns a grounding of a source.
    /// WARNING: adding duplicate groundings (for the same source) will result in a panic (in debug mode).
    ///
    /// Does not do anything relating to ground supports.
    fn post_ground_source(&mut self, source: Source, source_grounding: SourceGrounding) -> SourceGroundingId {
        #[cfg(debug_assertions)]
        debug_assert!(!self.groundings.contains_key(&(source, source_grounding.clone())));

        let source_grounding_id = self.groundings_rev.len();

        #[cfg(debug_assertions)]
        {
            self.groundings
                .insert((source, source_grounding.clone()), source_grounding_id);
        }
        self.groundings_rev.push((source, source_grounding));

        if let Some(task_id) = source {
            if !self.entries.1.contains_key(task_id) {
                self.entries.1.insert(task_id, vec![]);
            }
            self.entries.1[task_id].push(source_grounding_id);
        } else {
            self.entries.0.push(source_grounding_id);
        }
        source_grounding_id
    }
    pub fn get(&self, source: Source) -> &[SourceGroundingId] {
        if let Some(task_id) = source {
            debug_assert!(
                self.entries
                    .1
                    .get(task_id)
                    .is_none_or(|entries| entries.iter().all_unique())
            );
            self.entries
                .1
                .get(task_id)
                .map(|entries| entries.as_slice())
                .unwrap_or_default()
        } else {
            debug_assert!(self.entries.0.iter().all_unique());
            &self.entries.0
        }
    }
    // pub fn iter_all(&self) -> impl Iterator<Item = (Source, &Vec<SourceGroundingId>)> {
    //     debug_assert!(self.entries.0.iter().all_unique());
    //     debug_assert!(self.entries.1.iter().all(|entries| entries.1.iter().all_unique()));
    //
    //     std::iter::chain(
    //         [(None, &self.entries.0)],
    //         self.entries.1.iter().map(|(task_id, entries)| (Some(task_id), entries)),
    //     )
    // }
}

type FluentId = usize;
type StateVarGrounding = Vec<IntCst>;
type StateVarGroundingId = usize;

struct StateVarGroundingLookup<'a> {
    fluent_id: FluentId,
    state_var_grounding: &'a [IntCst],
}
impl<'a> hashbrown::Equivalent<(FluentId, StateVarGrounding)> for StateVarGroundingLookup<'a> {
    fn equivalent(&self, key: &(FluentId, StateVarGrounding)) -> bool {
        self.fluent_id == key.0 && self.state_var_grounding == key.1.as_slice()
    }
}
impl<'a> std::hash::Hash for StateVarGroundingLookup<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fluent_id.hash(state);
        self.state_var_grounding.hash(state);
    }
}
#[derive(Clone, Default)]
pub(super) struct StateVarsGroundingsInfo {
    fluents_ids: HashMap<crate::Sym, FluentId>,
    /// Stores ids of state variable groundings (they are used in transition grounding ids as the first element of the triple)
    entries: hashbrown::HashMap<(FluentId, StateVarGrounding), StateVarGroundingId>,

    #[cfg(debug_assertions)]
    groundings_rev: Vec<(FluentId, StateVarGrounding)>,
}

impl StateVarsGroundingsInfo {
    /// NOTE: will *not* panic if an already known grounding is given (unlike when interning source groundings).
    /// To the contrary, this is used to retrieve the id of the given state variable grounding, if it was interned.
    fn post_ground_state_var(&mut self, fluent: &crate::Sym, state_var_grounding: &[IntCst]) -> StateVarGroundingId {
        let fluent_id = if self.fluents_ids.contains_key(fluent) {
            *self.fluents_ids.get(fluent).unwrap()
        } else {
            let fluent_id = self.fluents_ids.len();
            self.fluents_ids.insert(fluent.into(), fluent_id);
            fluent_id
        };

        let lookup = StateVarGroundingLookup {
            fluent_id,
            state_var_grounding,
        };

        if let Some(&state_var_grounding_id) = self.entries.get(&lookup) {
            state_var_grounding_id
        } else {
            let state_var_grounding_id = self.entries.len();
            self.entries
                .insert((fluent_id, state_var_grounding.into()), state_var_grounding_id);
            #[cfg(debug_assertions)]
            {
                debug_assert!(state_var_grounding_id == self.groundings_rev.len());
                self.groundings_rev.push((fluent_id, state_var_grounding.into()));
            }
            state_var_grounding_id
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct TermsGroundingsInfo {
    /// Flat storage of term assignments with the ground transitions in which they appear.
    entries_merged_transitions: Vec<(IntTerm, IntCst, TransitionId, TransitionGroundingId)>,
    /// Same as for transitions, but for ground sources.
    entries_merged_sources: Vec<(IntTerm, IntCst, Source, SourceGroundingId)>,

    /// A buffer into which the entries are pushed before being deduped, sorted, and merged into the
    /// corresponding 'merged' field (see above).
    /// This could allow a more efficient incremental interning of new entries,
    /// via interleaved sorting then merging, rather than global merging every time.
    entries_pending_transitions: Vec<(IntTerm, IntCst, TransitionId, TransitionGroundingId)>,
    /// Same as for transitions.
    entries_pending_sources: Vec<(IntTerm, IntCst, Source, SourceGroundingId)>,
}
impl TermsGroundingsInfo {
    // Interns a grounding of a source.
    // WARNING: adding duplicate entries will result in a panic (in debug mode).
    //
    // Does not do anything relating to ground supports.
    fn post_for_ground_transition(
        &mut self,
        term: IntTerm,
        value: IntCst,
        transition_id: TransitionId,
        transition_grounding_id: TransitionGroundingId,
    ) {
        assert!(!term.is_cst());
        debug_assert!(!self.entries_merged_transitions.contains(&(
            term,
            value,
            transition_id,
            transition_grounding_id
        )));

        self.entries_pending_transitions
            .push((term, value, transition_id, transition_grounding_id));
    }
    fn post_for_ground_source(
        &mut self,
        term: IntTerm,
        value: IntCst,
        source: Source,
        source_grounding_id: SourceGroundingId,
    ) {
        assert!(!term.is_cst());
        debug_assert!(
            !self
                .entries_merged_sources
                .contains(&(term, value, source, source_grounding_id))
        );

        self.entries_pending_sources
            .push((term, value, source, source_grounding_id));
    }

    fn cmp_trs(
        x: &(IntTerm, IntCst, TransitionId, TransitionGroundingId),
        y: &(IntTerm, IntCst, TransitionId, TransitionGroundingId),
    ) -> std::cmp::Ordering {
        x.cmp(y)
    }
    fn cmp_src(
        x: &(IntTerm, IntCst, Source, SourceGroundingId),
        y: &(IntTerm, IntCst, Source, SourceGroundingId),
    ) -> std::cmp::Ordering {
        x.cmp(y)
    }

    fn sort_and_merge_pending_transitions(&mut self) {
        self.entries_pending_transitions.sort_unstable();
        self.entries_pending_transitions.dedup();
        merge_dedup_into(
            &mut self.entries_pending_transitions,
            &mut self.entries_merged_transitions,
            Self::cmp_trs,
        );
        debug_assert!(self.entries_pending_transitions.is_empty());
        debug_assert!(self.entries_merged_transitions.is_sorted());
    }
    fn sort_and_merge_pending_sources(&mut self) {
        self.entries_pending_sources.sort_unstable();
        self.entries_pending_sources.dedup();
        merge_dedup_into(
            &mut self.entries_pending_sources,
            &mut self.entries_merged_sources,
            Self::cmp_src,
        );
        debug_assert!(self.entries_pending_sources.is_empty());
        debug_assert!(self.entries_merged_sources.is_sorted());
    }

    fn debug_check_valid_for_transitions(&self) -> bool {
        debug_assert!(self.entries_pending_transitions.is_empty());
        debug_assert!(self.entries_merged_transitions.is_sorted());
        debug_assert!(self.entries_merged_transitions.iter().all_unique());
        true
    }
    fn debug_check_valid_for_sources(&self) -> bool {
        debug_assert!(self.entries_pending_sources.is_empty());
        debug_assert!(self.entries_merged_sources.is_sorted());
        debug_assert!(self.entries_merged_sources.iter().all_unique());
        true
    }

    /// Sorted, meaning the iterator can be chunked (without extra allocations).
    pub fn iter_sorted_all_for_transitions(
        &self,
    ) -> impl Iterator<Item = &(IntTerm, IntCst, TransitionId, TransitionGroundingId)> {
        debug_assert!(self.debug_check_valid_for_transitions());
        self.entries_merged_transitions.iter()
    }
    /// Sorted, meaning the iterator can be chunked (without extra allocations).
    pub fn iter_sorted_all_for_sources(&self) -> impl Iterator<Item = &(IntTerm, IntCst, Source, SourceGroundingId)> {
        debug_assert!(self.debug_check_valid_for_sources());
        self.entries_merged_sources.iter()
    }
    /// Sorted, meaning the iterator can be chunked (without extra allocations).
    pub fn iter_sorted_all_only_assignments(&self) -> impl Iterator<Item = (IntTerm, IntCst)> {
        debug_assert!(self.debug_check_valid_for_sources());
        self.entries_merged_sources
            .iter()
            .map(|(term, value, _, _)| (*term, *value))
            .dedup()
    }
}

#[derive(Clone, Default)]
pub(super) struct TransitionsGroundingsInfo {
    sources_of: DirectIdMap<TransitionId, Source>,

    /// Sorted
    entries_sourced_empty_source: Vec<(TransitionId, TransitionGroundingId, SourceGroundingId)>,
    /// Sorted
    entries_sourced_concrete_sources:
        DirectIdMap<crate::TaskId, Vec<(TransitionId, TransitionGroundingId, SourceGroundingId)>>,
}
impl TransitionsGroundingsInfo {
    fn post_ground_transition(
        &mut self,
        transition_id: TransitionId,
        transition_grounding_id: TransitionGroundingId,
        source: Source,
        source_grounding_id: SourceGroundingId,
    ) {
        if !self.sources_of.contains_key(transition_id) {
            self.sources_of.insert(transition_id, source);
        } else {
            assert!(self.sources_of[transition_id] == source);
        }

        if let Some(task_id) = source {
            if !self.entries_sourced_concrete_sources.contains_key(task_id) {
                self.entries_sourced_concrete_sources.insert(task_id, vec![]);
            }
            debug_assert!(!self.entries_sourced_concrete_sources[task_id].contains(&(
                transition_id,
                transition_grounding_id,
                source_grounding_id
            )));
            self.entries_sourced_concrete_sources[task_id].push((
                transition_id,
                transition_grounding_id,
                source_grounding_id,
            ));
        } else {
            debug_assert!(!self.entries_sourced_empty_source.contains(&(
                transition_id,
                transition_grounding_id,
                source_grounding_id
            )));
            self.entries_sourced_empty_source
                .push((transition_id, transition_grounding_id, source_grounding_id));
        }
    }

    fn debug_check_valid(&self) -> bool {
        debug_assert!(self.entries_sourced_empty_source.is_sorted());
        debug_assert!(self.entries_sourced_empty_source.iter().all_unique());
        debug_assert!(
            self.entries_sourced_concrete_sources
                .iter()
                .all(|(_, entries)| entries.is_sorted())
        );
        debug_assert!(
            self.entries_sourced_concrete_sources
                .iter()
                .all(|(_, entries)| entries.iter().all_unique())
        );
        true
    }

    // fn sort_for(&mut self, source: Source) {
    //     if let Some(task_id) = source {
    //         if self.entries_sourced_concrete_sources.contains_key(task_id) {
    //             self.entries_sourced_concrete_sources[task_id].sort_unstable();
    //         }
    //     } else {
    //         self.entries_sourced_empty_source
    //             .sort_unstable();
    //     }
    // }
    pub fn sort_for_all(&mut self) {
        self.entries_sourced_empty_source.sort_unstable();
        for (_, entries) in self.entries_sourced_concrete_sources.iter_mut() {
            entries.sort_unstable();
        }
    }

    pub fn iter_all_sourced(
        &self,
    ) -> impl Iterator<
        Item = (
            Source,
            impl Iterator<Item = &(TransitionId, TransitionGroundingId, SourceGroundingId)>,
        ),
    > {
        debug_assert!(self.debug_check_valid());
        std::iter::chain(
            [(None, self.entries_sourced_empty_source.iter())],
            self.entries_sourced_concrete_sources
                .iter()
                .map(|(source, entries)| (Some(source), entries.iter())),
        )
    }

    /// WARNING: although the the iterator can be chunked (without extra allocations) by transition, it may not necessarily be *sorted* !!
    pub fn iter_all_unsourced(&self) -> impl Iterator<Item = (TransitionId, TransitionGroundingId)> {
        debug_assert!(self.debug_check_valid());
        // result is not necessarily sorted, but chunked because a transition is "owned" only by a single source
        std::iter::chain(
            self.entries_sourced_empty_source
                .iter()
                .map(|(transition_id, transition_grounding_id, _)| (*transition_id, *transition_grounding_id)),
            self.entries_sourced_concrete_sources.iter().flat_map(|(_, entries)| {
                entries
                    .iter()
                    .map(|(transition_id, transition_grounding_id, _)| (*transition_id, *transition_grounding_id))
            }),
        )
        .dedup()
    }

    /// Returns the (sub)slice with the groundings of the given transition (if there are any)
    /// Note that the groundings in the slice are ordered (notably meaning that it is chunkable)
    ///
    /// If the slice cannot be found, returns an empty one.
    fn get_index_and_slice(
        &self,
        transition_id: TransitionId,
    ) -> &[(TransitionId, TransitionGroundingId, SourceGroundingId)] {
        debug_assert!(self.debug_check_valid());

        let Some(&source) = self.sources_of.get(transition_id) else {
            return [].as_slice();
        };
        let entries = if let Some(task_id) = source {
            &self.entries_sourced_concrete_sources[task_id]
        } else {
            &self.entries_sourced_empty_source
        };
        binary_search_range_by(entries, |(transition_id_, _, _)| (*transition_id_).cmp(&transition_id))
            .map_or_else(|| [].as_slice(), |(i, j)| &entries[i..j])
    }
}

#[derive(Clone, Default)]
pub(super) struct SupportsGroundingsInfo {
    /// Sorted flat storage of ground supports: "outgoing view":
    /// (out_transition_id, out_transition_grounding_id, in_transition_id, in_transition_grounding_id).
    out: Vec<(TransitionId, TransitionGroundingId, TransitionId, TransitionGroundingId)>,
    /// Sorted flat storage of ground supports: "incoming view":
    /// (in_transition_id, in_transition_grounding_id, out_transition_id, out_transition_grounding_id).
    in_: Vec<(TransitionId, TransitionGroundingId, TransitionId, TransitionGroundingId)>,
    /// Sorted flat storage of ground supports: "neutral view":
    /// (out_transition_id, in_transition_id, out_transition_grounding_id, in_transition_grounding_id).
    entries: Vec<(TransitionId, TransitionId, TransitionGroundingId, TransitionGroundingId)>,
}

impl SupportsGroundingsInfo {
    pub fn from(
        lifted_supports: &LpRelaxEncodingLiftedSupportsSorted,
        transitions_groundings: &TransitionsGroundingsInfo,
    ) -> Self {
        debug_assert!(transitions_groundings.debug_check_valid());

        let mut out = vec![];
        let mut in_ = vec![];
        let mut entries = vec![];

        for &(out_transition_id, in_transition_id) in lifted_supports.out() {
            let out_slice = transitions_groundings.get_index_and_slice(out_transition_id);
            let in_slice = transitions_groundings.get_index_and_slice(in_transition_id);

            debug_assert!(
                out_slice
                    .iter()
                    .all(|&(transition_id, _, _)| transition_id == out_transition_id),
            );
            debug_assert!(
                in_slice
                    .iter()
                    .all(|&(transition_id, _, _)| transition_id == in_transition_id)
            );

            debug_assert!(
                out_slice
                    .iter()
                    .all(|(_, out_transition_grounding_id, _)| out_transition_grounding_id.valto.is_some()),
                "out transition mustn't be a condition"
            );

            if out_slice.is_empty() || in_slice.is_empty() {
                continue;
            }

            let mut push_supports =
                |out_transition_grounding_id: TransitionGroundingId,
                 in_transition_grounding_id: TransitionGroundingId| {
                    entries.push((
                        out_transition_id,
                        in_transition_id,
                        out_transition_grounding_id,
                        in_transition_grounding_id,
                    ));
                    out.push((
                        out_transition_id,
                        out_transition_grounding_id,
                        in_transition_id,
                        in_transition_grounding_id,
                    ));
                    in_.push((
                        in_transition_id,
                        in_transition_grounding_id,
                        out_transition_id,
                        out_transition_grounding_id,
                    ));
                };

            let in_transition_is_eff = in_slice
                .first()
                .is_some_and(|(_, in_transition_grounding_id, _)| in_transition_grounding_id.valfrom.is_none());

            let out_direct = out_slice
                .iter()
                .map(|&(_, transition_grounding_id, _)| transition_grounding_id)
                .dedup()
                .collect_vec();
            debug_assert!(out_direct.is_sorted());
            debug_assert!(out_direct.iter().all_unique());

            let in_direct = in_slice
                .iter()
                .map(|&(_, transition_grounding_id, _)| transition_grounding_id)
                .dedup()
                .collect_vec();
            debug_assert!(in_direct.is_sorted());
            debug_assert!(in_direct.iter().all_unique());

            // Case: in-transition ("consumer" of support) is an pure-effect (i.e. valfrom is None for all its groundings).
            // Any other (non-pure-cond) ground transition (with the same state var grounding) can support it, whatever its valto

            if in_transition_is_eff {
                for (out_chunk_same_sv, in_chunk_same_sv) in
                    merge_join_chunks_by_key(&out_direct, &in_direct, |transition_grounding_id| {
                        transition_grounding_id.state_var_grounding_id
                    })
                {
                    for (&out_transition_grounding_id, &in_transition_grounding_id) in
                        out_chunk_same_sv.iter().cartesian_product(in_chunk_same_sv.iter())
                    {
                        push_supports(out_transition_grounding_id, in_transition_grounding_id);
                    }
                }
                continue;
            }

            // Case: in-transition ("consumer" of support) is *not* a pure-effect (i.e. valfrom is not None for all its groundings).
            // it can be support by other (non-pure-cond) ground transitions (with the same state var grounding) and out_valto = in_valfrom

            debug_assert!(!in_transition_is_eff);
            debug_assert!(
                in_direct
                    .iter()
                    .all(|transition_grounding_id| transition_grounding_id.valfrom.is_some())
            );

            let in_direct = in_direct
                .into_iter()
                .map(|transition_grounding_id| {
                    (
                        transition_grounding_id.state_var_grounding_id,
                        transition_grounding_id.valfrom,
                        transition_grounding_id.valto,
                    )
                })
                .collect_vec();

            let out_inverted = out_direct
                .into_iter()
                .map(|transition_grounding_id| {
                    (
                        transition_grounding_id.state_var_grounding_id,
                        transition_grounding_id.valto,
                        transition_grounding_id.valfrom,
                    )
                })
                .sorted_unstable()
                .collect_vec();
            debug_assert!(out_inverted.is_sorted());
            debug_assert!(out_inverted.iter().all_unique());

            for (out_inv_chunk_same_sv, in_chunk_same_sv) in
                merge_join_chunks_by_key(&out_inverted, &in_direct, |&(state_var_grounding_id, _, _)| {
                    state_var_grounding_id
                })
            {
                // Within a same-state-var chunk, `out_inverted` is sorted by valto and `in_direct` by
                // valfrom, so the same join applies to the values.
                for (out_inv_chunk_matching, in_chunk_matching) in merge_join_chunks_by_key(
                    out_inv_chunk_same_sv,
                    in_chunk_same_sv,
                    |&(_, value, _)| {
                        value.expect(
                            "'out' side: `value` is 'out_valto' (never None as an out transition is never a pure condition).\
                            'in' side: `value` is 'in_valfrom' (never None as the pure-effect case was already handled above)"
                        )
                    },
                ) {
                    for (&out_inverted_transition_grounding_id, &in_transition_grounding_id) in out_inv_chunk_matching
                        .iter()
                        .cartesian_product(in_chunk_matching.iter())
                    {
                        let in_transition_grounding_id = TransitionGroundingId {
                            state_var_grounding_id: in_transition_grounding_id.0,
                            valfrom: in_transition_grounding_id.1,
                            valto: in_transition_grounding_id.2,
                        };
                        // un-invert: (sv, valto, valfrom) -> (sv, valfrom, valto)
                        let out_transition_grounding_id = TransitionGroundingId {
                            state_var_grounding_id: out_inverted_transition_grounding_id.0,
                            valfrom: out_inverted_transition_grounding_id.2,
                            valto: out_inverted_transition_grounding_id.1,
                        };

                        push_supports(out_transition_grounding_id, in_transition_grounding_id);
                    }
                }
            }
        }

        debug_assert!(entries.iter().all_unique());
        debug_assert!(entries.is_sorted());
        //entries.sort_unstable();

        debug_assert!(out.iter().all_unique());
        out.sort_unstable();

        debug_assert!(in_.iter().all_unique());
        in_.sort_unstable();

        Self { entries, out, in_ }
    }

    pub fn iter_all(
        &self,
    ) -> impl Iterator<Item = &(TransitionId, TransitionId, TransitionGroundingId, TransitionGroundingId)> {
        self.entries.iter()
    }
    pub fn iter_out_all(
        &self,
    ) -> impl Iterator<Item = &(TransitionId, TransitionGroundingId, TransitionId, TransitionGroundingId)> {
        self.out.iter()
    }
    pub fn iter_in_all(
        &self,
    ) -> impl Iterator<Item = &(TransitionId, TransitionGroundingId, TransitionId, TransitionGroundingId)> {
        self.in_.iter()
    }
}

#[cfg(test)]
mod tests {
    use idmap::intid::IntegerId;

    use crate::TaskId;

    use super::*;

    #[test]
    fn test_sources_groundings_addition() {
        let mut sources_groundings = SourcesGroundingsInfo::default();

        sources_groundings.post_ground_source(Some(TaskId::from_int(1)), SourceGrounding(vec![1000, 50, 40]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(1)), SourceGrounding(vec![1000, 50, 30]));

        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), SourceGrounding(vec![2, 10, 400, 300]));

        sources_groundings.post_ground_source(None, SourceGrounding(vec![]));

        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), SourceGrounding(vec![2, 10, 401, 300]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), SourceGrounding(vec![2, 10, 400, 301]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), SourceGrounding(vec![2, 10, 401, 301]));

        assert!(
            sources_groundings.entries == {
                let mut res = DirectIdMap::new();
                res.insert(TaskId::from_int(0), vec![2, 4, 5, 6]);
                res.insert(TaskId::from_int(1), vec![0, 1]);
                (vec![3], res)
            }
        );
    }

    #[test]
    fn test_sources_groundings_panic_on_dupe() {
        fn catch_unwind_silent<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) -> std::thread::Result<R> {
            let prev_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let result = std::panic::catch_unwind(f);
            std::panic::set_hook(prev_hook);
            result
        }
        let result = catch_unwind_silent(|| {
            let mut sources_groundings = SourcesGroundingsInfo::default();

            sources_groundings.post_ground_source(Some(TaskId::from_int(0)), SourceGrounding(vec![1, 1, 1]));
            sources_groundings.post_ground_source(Some(TaskId::from_int(0)), SourceGrounding(vec![1, 1, 1]));
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_state_vars_groundings() {
        let mut state_vars_groundings = StateVarsGroundingsInfo::default();

        assert_eq!(
            state_vars_groundings.post_ground_state_var(&"a".to_string(), &[1, 2, 3, 4]),
            0
        );
        assert_eq!(
            state_vars_groundings.post_ground_state_var(&"b".to_string(), &[1, 2, 3, 4]),
            1
        );
        assert_eq!(
            state_vars_groundings.post_ground_state_var(&"b".to_string(), &[1, 2, 3, 5]),
            2
        );
        assert_eq!(
            state_vars_groundings.post_ground_state_var(&"a".to_string(), &[1, 2, 3, 4]),
            0
        );
    }

    #[test]
    fn test_terms_groundings() {
        // TODO
    }
}
