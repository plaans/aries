mod utils;

use utils::{binary_search_range_by, merge_dedup_into};

use std::collections::HashMap;

use aries_solver::core::IntCst;
use idmap::DirectIdMap;
use itertools::Itertools;

use crate::IntTerm;
use crate::analysis::Source;
use crate::analysis::transitions::TransitionId;
use crate::analysis::transitions::supports::SupportsSorted;
use utils::merge_join_chunks_by_key;

pub type SourceGrounding = crate::analysis::grounding::ParametersAssignment;
pub type SourceGroundingId = usize;

#[derive(Clone, Default)]
pub(crate) struct SourcesGroundingsInfo {
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
    pub fn post_ground_source(&mut self, source: Source, source_grounding: &SourceGrounding) -> SourceGroundingId {
        #[cfg(debug_assertions)]
        debug_assert!(!self.groundings.contains_key(&(source, source_grounding.clone())));

        let source_grounding_id = self.groundings_rev.len();

        #[cfg(debug_assertions)]
        {
            self.groundings
                .insert((source, source_grounding.clone()), source_grounding_id);
        }
        self.groundings_rev.push((source, source_grounding.clone()));

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
}

type FluentId = usize;
type StateVarGrounding = smallvec::SmallVec<[IntCst; 4]>;
pub type StateVarGroundingId = usize;

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
pub(crate) struct StateVarsGroundingsInfo {
    fluents_ids: HashMap<crate::Sym, FluentId>,
    /// Stores ids of state variable groundings (they are used in transition grounding ids as the first element of the triple)
    entries: hashbrown::HashMap<(FluentId, StateVarGrounding), StateVarGroundingId>,

    #[cfg(debug_assertions)]
    groundings_rev: Vec<(FluentId, StateVarGrounding)>,
}
impl StateVarsGroundingsInfo {
    /// NOTE: will *not* panic if an already known grounding is given (unlike when interning source groundings).
    /// To the contrary, this is used to retrieve the id of the given state variable grounding, if it was interned.
    pub fn post_ground_state_var(
        &mut self,
        fluent: &crate::Sym,
        state_var_grounding: &[IntCst],
    ) -> StateVarGroundingId {
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
pub(crate) struct TermsGroundingsInfo {
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
    pub fn post_for_ground_transition(
        &mut self,
        term: IntTerm,
        value: IntCst,
        trans_id: TransitionId,
        trans_grounding_id: TransitionGroundingId,
    ) {
        assert!(!term.is_cst());
        debug_assert!(
            !self
                .entries_merged_transitions
                .contains(&(term, value, trans_id, trans_grounding_id))
        );

        self.entries_pending_transitions
            .push((term, value, trans_id, trans_grounding_id));
    }
    pub fn post_for_ground_source(
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

    pub fn sort(&mut self) {
        self.sort_and_merge_pending_transitions();
        self.sort_and_merge_pending_sources();
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
        debug_assert!(is_sorted_and_no_dupes(self.entries_merged_transitions.iter()));
        true
    }
    fn debug_check_valid_for_sources(&self) -> bool {
        debug_assert!(self.entries_pending_sources.is_empty());
        debug_assert!(is_sorted_and_no_dupes(self.entries_merged_sources.iter()));
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransitionGroundingId {
    pub(super) state_var_grounding_id: usize,
    pub(super) val_assignment: Option<IntCst>,
    pub(super) op_assignment: Option<IntCst>,
}
impl TransitionGroundingId {
    fn is_pure_eff(&self) -> bool {
        self.op_assignment.is_some() && self.val_assignment.is_none()
    }
}
#[derive(Clone, Default)]
pub(crate) struct TransitionsGroundingsInfo {
    sources_of: DirectIdMap<TransitionId, Source>,

    /// Sorted
    entries_sourced_empty_source: Vec<(TransitionId, TransitionGroundingId, SourceGroundingId)>,
    /// Sorted
    entries_sourced_concrete_sources:
        DirectIdMap<crate::TaskId, Vec<(TransitionId, TransitionGroundingId, SourceGroundingId)>>,
}
impl TransitionsGroundingsInfo {
    pub fn post_ground_transition(
        &mut self,
        trans_id: TransitionId,
        trans_grounding_id: TransitionGroundingId,
        source: Source,
        source_grounding_id: SourceGroundingId,
    ) {
        if !self.sources_of.contains_key(trans_id) {
            self.sources_of.insert(trans_id, source);
        } else {
            assert!(self.sources_of[trans_id] == source);
        }

        if let Some(task_id) = source {
            if !self.entries_sourced_concrete_sources.contains_key(task_id) {
                self.entries_sourced_concrete_sources.insert(task_id, vec![]);
            }
            debug_assert!(!self.entries_sourced_concrete_sources[task_id].contains(&(
                trans_id,
                trans_grounding_id,
                source_grounding_id
            )));
            self.entries_sourced_concrete_sources[task_id].push((trans_id, trans_grounding_id, source_grounding_id));
        } else {
            debug_assert!(!self.entries_sourced_empty_source.contains(&(
                trans_id,
                trans_grounding_id,
                source_grounding_id
            )));
            self.entries_sourced_empty_source
                .push((trans_id, trans_grounding_id, source_grounding_id));
        }
    }

    fn debug_check_valid(&self) -> bool {
        debug_assert!(is_sorted_and_no_dupes(self.entries_sourced_empty_source.iter()));
        debug_assert!(is_sorted_and_no_dupes(self.entries_sourced_concrete_sources.iter()));
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

    fn source_of(&self, trans_id: TransitionId) -> Source {
        self.sources_of[trans_id]
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
                .map(|(trans_id, trans_grounding_id, _)| (*trans_id, *trans_grounding_id)),
            self.entries_sourced_concrete_sources.iter().flat_map(|(_, entries)| {
                entries
                    .iter()
                    .map(|(trans_id, trans_grounding_id, _)| (*trans_id, *trans_grounding_id))
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
        trans_id: TransitionId,
    ) -> &[(TransitionId, TransitionGroundingId, SourceGroundingId)] {
        debug_assert!(self.debug_check_valid());

        let Some(&source) = self.sources_of.get(trans_id) else {
            return [].as_slice();
        };
        let entries = if let Some(task_id) = source {
            &self.entries_sourced_concrete_sources[task_id]
        } else {
            &self.entries_sourced_empty_source
        };
        binary_search_range_by(entries, |(trans_id_, _, _)| (*trans_id_).cmp(&trans_id))
            .map_or_else(|| [].as_slice(), |(i, j)| &entries[i..j])
    }
}

#[derive(Clone, Default)]
pub(crate) struct SupportsGroundingsInfo {
    /// Sorted flat storage of ground supports: "outgoing view":
    /// (out_trans_id, out_trans_grounding_id, (in_trans_id, in_trans_grounding_id)).
    out: Vec<(
        TransitionId,
        TransitionGroundingId,
        (TransitionId, TransitionGroundingId),
    )>,
    /// Sorted flat storage of ground supports: "incoming view":
    /// (in_trans_id, in_trans_grounding_id, (out_trans_id, out_trans_grounding_id)).
    #[allow(clippy::type_complexity)]
    in_: Vec<(
        TransitionId,
        TransitionGroundingId,
        Option<(TransitionId, TransitionGroundingId)>,
    )>,
    /// Sorted (over the first 2 element) flat storage of ground supports: "neutral view".
    /// (out_trans_id, in_trans_id, (out_trans_grounding_id, in_trans_grounding_id)).
    #[allow(clippy::type_complexity)]
    entries: Vec<(
        TransitionId,
        TransitionId,
        Option<(TransitionGroundingId, TransitionGroundingId)>,
    )>,
}

impl SupportsGroundingsInfo {
    pub fn from(supports_sorted: &SupportsSorted, transitions_groundings: &TransitionsGroundingsInfo) -> Self {
        debug_assert!(transitions_groundings.debug_check_valid());

        let mut out = vec![];
        let mut in_ = vec![];
        let mut entries = vec![];

        // (Pre-seeding) Acknowledge that an in-transition and a grounding of it exist.
        // Later, after (in_trans_id, in_trans_grounding_id, Some(..)) entries are added,
        // the pre-seeded entry (in_trans_id, in_trans_grounding_id, None) will be removed.
        // If no (in_trans_id, in_trans_grounding_id, Some(..)) entries are added,
        // then the (in_trans_id, in_trans_grounding_id, None) is kept and signifies
        // that although the in-transition and the grounding exist, there exist no compatible out-transitions to support it.
        in_.extend(
            transitions_groundings
                .iter_all_unsourced()
                .filter_map(|(trans_id, trans_grounding_id)| {
                    let trans_is_non_initial_eff =
                        !trans_grounding_id.is_pure_eff() || transitions_groundings.source_of(trans_id).is_some();
                    trans_is_non_initial_eff.then_some((trans_id, trans_grounding_id, None))
                }),
        );

        // Main loop
        for &((out_trans_id, in_trans_id), _) in supports_sorted.sorted_out() {
            let out_slice = transitions_groundings.get_index_and_slice(out_trans_id);
            let in_slice = transitions_groundings.get_index_and_slice(in_trans_id);

            let entries_len_before_search = entries.len();

            let mut push_supports = |out_trans_grounding_id: TransitionGroundingId,
                                     in_trans_grounding_id: TransitionGroundingId| {
                out.push((
                    out_trans_id,
                    out_trans_grounding_id,
                    (in_trans_id, in_trans_grounding_id),
                ));
                in_.push((
                    in_trans_id,
                    in_trans_grounding_id,
                    Some((out_trans_id, out_trans_grounding_id)),
                ));
                entries.push((
                    out_trans_id,
                    in_trans_id,
                    Some((out_trans_grounding_id, in_trans_grounding_id)),
                ));
            };

            // debug_assert!(
            //     out_slice
            //         .iter()
            //         .all(|&(trans_id, _, _)| trans_id == out_trans_id),
            // );
            // debug_assert!(
            //     in_slice
            //         .iter()
            //         .all(|&(trans_id, _, _)| trans_id == in_trans_id)
            // );

            'search: {
                let in_transition_is_eff = in_slice
                    .first()
                    .is_some_and(|(_, in_trans_grounding_id, _)| in_trans_grounding_id.is_pure_eff());

                let out_direct: Vec<TransitionGroundingId> = out_slice
                    .iter()
                    .map(|&(_, trans_grounding_id, _)| trans_grounding_id)
                    .dedup()
                    .collect_vec();
                debug_assert!(is_sorted_and_no_dupes(out_direct.iter()));

                let in_direct: Vec<TransitionGroundingId> = in_slice
                    .iter()
                    .map(|&(_, trans_grounding_id, _)| trans_grounding_id)
                    .dedup()
                    .collect_vec();
                debug_assert!(is_sorted_and_no_dupes(in_direct.iter()));

                // Case: in-transition ("consumer" of support) is an pure-effect (i.e. `val_assignment` is None for all its groundings).
                // Any other (non-pure-cond) ground transition (with the same state var grounding) can support it, whatever its `op_assignment`

                if in_transition_is_eff {
                    for (out_chunk_same_sv, in_chunk_same_sv) in
                        merge_join_chunks_by_key(&out_direct, &in_direct, |trans_grounding_id| {
                            trans_grounding_id.state_var_grounding_id
                        })
                    {
                        for (&out_trans_grounding_id, &in_trans_grounding_id) in
                            out_chunk_same_sv.iter().cartesian_product(in_chunk_same_sv.iter())
                        {
                            push_supports(out_trans_grounding_id, in_trans_grounding_id);
                        }
                    }
                    break 'search;
                }

                // Case: in-transition ("consumer" of support) is *not* a pure-effect (i.e. val_assignment is not None for all its groundings).
                // it can be support by other (non-pure-cond) ground transitions (with the same state var grounding) and out_op_assignment = `in_val_assignment

                debug_assert!(!in_transition_is_eff);

                let in_direct: Vec<(StateVarGroundingId, Option<IntCst>, Option<IntCst>)> = in_direct
                    .into_iter()
                    .map(|trans_grounding_id| {
                        (
                            trans_grounding_id.state_var_grounding_id,
                            trans_grounding_id.val_assignment,
                            trans_grounding_id.op_assignment,
                        )
                    })
                    .collect_vec();
                debug_assert!(is_sorted_and_no_dupes(in_direct.iter()));

                let out_inverted: Vec<(StateVarGroundingId, Option<IntCst>, Option<IntCst>)> = out_direct
                    .into_iter()
                    .map(|trans_grounding_id| {
                        (
                            trans_grounding_id.state_var_grounding_id,
                            trans_grounding_id.op_assignment,
                            trans_grounding_id.val_assignment,
                        )
                    })
                    .sorted_unstable()
                    .collect_vec();
                debug_assert!(is_sorted_and_no_dupes(out_inverted.iter()));

                for (out_inv_chunk_same_sv, in_chunk_same_sv) in
                    merge_join_chunks_by_key(&out_inverted, &in_direct, |&(state_var_grounding_id, _, _)| {
                        state_var_grounding_id
                    })
                {
                    // Within a same-state-var chunk, `out_inverted` is sorted by `op_assignment` and `in_direct` by
                    // `val_assignment, so the same join applies to the values.
                    for (out_inv_chunk_matching, in_chunk_matching) in merge_join_chunks_by_key(
                        out_inv_chunk_same_sv,
                        in_chunk_same_sv,
                        |&(_, value, _)| {
                            value.expect(
                                "'out' side: `value` is 'out_op_assignment' (never None as an out transition is never a pure condition).
                                'in' side: `value` is 'in_val_assignment' (never None as the pure-effect case was already handled above)."
                            )
                        },
                    ) {
                        for (&out_inverted_trans_grounding_id, &in_trans_grounding_id) in out_inv_chunk_matching
                            .iter()
                            .cartesian_product(in_chunk_matching.iter())
                        {
                            let in_trans_grounding_id = TransitionGroundingId {
                                state_var_grounding_id: in_trans_grounding_id.0,
                                val_assignment: in_trans_grounding_id.1,
                                op_assignment: in_trans_grounding_id.2,
                            };
                            // un-invert
                            let out_trans_grounding_id = TransitionGroundingId {
                                state_var_grounding_id: out_inverted_trans_grounding_id.0,
                                val_assignment: out_inverted_trans_grounding_id.2,
                                op_assignment: out_inverted_trans_grounding_id.1,
                            };

                            push_supports(out_trans_grounding_id, in_trans_grounding_id);
                        }
                    }
                }
            }

            if entries.len() == entries_len_before_search {
                // Nothing was added: it means there no compatible groundings were found
                entries.push((out_trans_id, in_trans_id, None));
            }
        }

        debug_assert!(entries.is_sorted_by_key(|(out_trans_id, in_trans_id, _)| { (out_trans_id, in_trans_id) }));
        debug_assert!(
            // an entry with `None` exists iff there are no entries with Some (for the same (out_trans_id, in_trans_id) pair)
            entries
                .chunk_by(|a, b| (a.0, a.1) == (b.0, b.1))
                .all(|chunk| chunk.len() == 1 || chunk.iter().all(|e| e.2.is_some()))
        );
        debug_assert!(entries.iter().all_unique());

        debug_assert!(out.iter().all_unique());
        out.sort_unstable();

        debug_assert!(in_.iter().all_unique());
        in_.sort_unstable();

        // Remove pre-seeded (in_trans_id, in_trans_grounding_id, None) entries (see beginning)
        // if (in_trans_id, in_trans_grounding_id, Some(..)) entries were found / added during the search.
        in_.dedup_by(|next, prev| {
            if prev.2.is_none() && (prev.0, prev.1) == (next.0, next.1) {
                *prev = *next; // overwrite the sentinel with the support, then drop the duplicate
                true
            } else {
                false
            }
        });
        debug_assert!(
            // an entry with `None` exists iff there are no entries with Some (for the same (in_trans_id, in_trans_grounding_id) pair)
            in_.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1))
                .all(|chunk| chunk.len() == 1 || chunk.iter().all(|e| e.2.is_some()))
        );

        Self { entries, out, in_ }
    }

    pub fn iter_all(
        &self,
    ) -> impl Iterator<
        Item = &(
            TransitionId,
            TransitionId,
            Option<(TransitionGroundingId, TransitionGroundingId)>,
        ),
    > {
        self.entries.iter()
    }
    pub fn iter_out_all(
        &self,
    ) -> impl Iterator<
        Item = &(
            TransitionId,
            TransitionGroundingId,
            (TransitionId, TransitionGroundingId),
        ),
    > {
        self.out.iter()
    }
    pub fn iter_in_all(
        &self,
    ) -> impl Iterator<
        Item = &(
            TransitionId,
            TransitionGroundingId,
            Option<(TransitionId, TransitionGroundingId)>,
        ),
    > {
        self.in_.iter()
    }
}

fn is_sorted_and_no_dupes<T: Ord>(iter: impl Iterator<Item = T>) -> bool {
    iter.is_sorted_by(|a, b| a < b)
}

#[cfg(test)]
mod tests {
    use idmap::intid::IntegerId;

    use crate::TaskId;

    use super::*;

    #[test]
    fn test_sources_groundings_addition() {
        let mut sources_groundings = SourcesGroundingsInfo::default();

        sources_groundings.post_ground_source(Some(TaskId::from_int(1)), &SourceGrounding::from(vec![1000, 50, 40]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(1)), &SourceGrounding::from(vec![1000, 50, 30]));

        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![2, 10, 400, 300]));

        sources_groundings.post_ground_source(None, &SourceGrounding::from(vec![]));

        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![2, 10, 401, 300]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![2, 10, 400, 301]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![2, 10, 401, 301]));

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

            sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![1, 1, 1]));
            sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![1, 1, 1]));
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
