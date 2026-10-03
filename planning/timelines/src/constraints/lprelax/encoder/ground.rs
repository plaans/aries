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

/// A ground support between two transitions, which only depends on the state variable grounding and on the value handed over.
///
/// Every grounding of the out-transition with that state variable grounding and `op_assignment` (whatever its `val_assignment`)
/// can support every grounding of the in-transition with that state variable grounding and `val_assignment`
/// (whatever its `op_assignment`, and also, for a pure effect, whatever its `val_assignment`):
///
/// The carried value is that of the out-transition's `op_assignment`.
pub type SupportGroundingId = (StateVarGroundingId, IntCst);

/// The groundings of an in-transition that can receive the same ground supports:
/// (state var grounding, `val_assignment`), the latter being `None` for a pure effect.
pub type InGroupKey = (StateVarGroundingId, Option<IntCst>);

/// An entry of a group of groundings (of a same transition) in the outgoing / incoming views of [`SupportsGroundingsInfo`].
/// Within a group, the groundings come first, then the ground supports.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GroupEntry<S> {
    /// One of the (distinct) groundings of the group.
    Grounding(TransitionGroundingId),
    /// A ground support out of / into the group.
    Support(S),
}

#[derive(Clone, Default)]
pub(crate) struct SupportsGroundingsInfo {
    /// Sorted flat storage of ground supports: "outgoing view":
    /// (out_trans_id, support_grounding_id, grounding of out_trans_id | in_trans_id).
    out: Vec<(TransitionId, SupportGroundingId, GroupEntry<TransitionId>)>,
    /// Sorted flat storage of ground supports: "incoming view":
    /// (in_trans_id, in_group_key, grounding of in_trans_id | (out_trans_id, support_grounding_id)).
    #[allow(clippy::type_complexity)]
    in_: Vec<(TransitionId, InGroupKey, GroupEntry<(TransitionId, SupportGroundingId)>)>,
    /// Sorted (over the first 2 element) flat storage of ground supports: "neutral view".
    /// (out_trans_id, in_trans_id, support_grounding_id).
    entries: Vec<(TransitionId, TransitionId, Option<SupportGroundingId>)>,
}

impl SupportsGroundingsInfo {
    pub fn from(supports_sorted: &SupportsSorted, transitions_groundings: &TransitionsGroundingsInfo) -> Self {
        debug_assert!(transitions_groundings.debug_check_valid());

        // The groundings of all the transitions that may need support (i.e. all but initial pure effects),
        // grouped by the ground supports they can receive.
        // A group that nothing can support is left with only these entries.
        let mut in_ = transitions_groundings
            .iter_all_unsourced()
            .filter(|&(trans_id, trans_grounding_id)| {
                !trans_grounding_id.is_pure_eff() || transitions_groundings.source_of(trans_id).is_some()
            })
            .map(|(trans_id, trans_grounding_id)| {
                (
                    trans_id,
                    (
                        trans_grounding_id.state_var_grounding_id,
                        trans_grounding_id.val_assignment,
                    ),
                    GroupEntry::Grounding(trans_grounding_id),
                )
            })
            .sorted_unstable()
            .collect::<Vec<_>>();

        // The groups of the in-transitions, to be joined with those of the out-transitions.
        let in_groups = in_
            .iter()
            .map(|&(in_trans_id, in_group_key, _)| (in_trans_id, in_group_key))
            .dedup()
            .collect::<Vec<_>>();

        let mut out = vec![];
        let mut entries = vec![];

        // Main loop
        let chunkby = supports_sorted
            .sorted_out()
            .iter()
            .chunk_by(|&&((out_trans_id, _), _)| out_trans_id);

        for (out_trans_id, lifted_supports) in &chunkby {
            // The groundings of the out-transition, grouped by the ground support they can hand over.
            let out_len_before = out.len();
            out.extend(
                transitions_groundings
                    .get_index_and_slice(out_trans_id)
                    .iter()
                    .map(|&(_, trans_grounding_id, _)| {
                        let support_grounding_id: SupportGroundingId = (
                            trans_grounding_id.state_var_grounding_id,
                            trans_grounding_id.op_assignment.unwrap(),
                        );
                        (
                            out_trans_id,
                            support_grounding_id,
                            GroupEntry::Grounding(trans_grounding_id),
                        )
                    })
                    .dedup(),
            );

            // The values the out-transition can hand over, by state variable grounding,
            // shaped as an `InGroupKey` to be joined with those of the in-transitions.
            let out_keys: Vec<InGroupKey> = out[out_len_before..]
                .iter()
                .map(|&(_, (state_var_grounding_id, value), _)| (state_var_grounding_id, Some(value)))
                .sorted_unstable()
                .dedup()
                .collect::<Vec<_>>();

            for &((_, in_trans_id), _) in lifted_supports {
                let in_keys: Vec<InGroupKey> =
                    binary_search_range_by(&in_groups, |(trans_id, _)| trans_id.cmp(&in_trans_id))
                        .map_or(&[][..], |(i, j)| &in_groups[i..j])
                        .iter()
                        .map(|&(_, in_group_key)| in_group_key)
                        .collect::<Vec<_>>();

                let entries_len_before_search = entries.len();

                let mut push_support = |(state_var_grounding_id, value): InGroupKey, in_group_key: InGroupKey| {
                    let support_grounding_id: SupportGroundingId = (state_var_grounding_id, value.unwrap());
                    out.push((out_trans_id, support_grounding_id, GroupEntry::Support(in_trans_id)));
                    in_.push((
                        in_trans_id,
                        in_group_key,
                        GroupEntry::Support((out_trans_id, support_grounding_id)),
                    ));
                    entries.push((out_trans_id, in_trans_id, Some(support_grounding_id)));
                };

                let in_trans_is_pure_eff = in_keys
                    .first()
                    .is_some_and(|&(_, val_assignment)| val_assignment.is_none());

                if in_trans_is_pure_eff {
                    // A pure effect needs no value: any value handed over on its state variable grounding supports it.
                    for (out_chunk_same_sv, in_chunk_same_sv) in
                        merge_join_chunks_by_key(&out_keys, &in_keys, |&(state_var_grounding_id, _)| {
                            state_var_grounding_id
                        })
                    {
                        debug_assert_eq!(in_chunk_same_sv.len(), 1);
                        for &out_key in out_chunk_same_sv {
                            push_support(out_key, in_chunk_same_sv[0]);
                        }
                    }
                } else {
                    // Any other in-transition needs the value handed over to be its `val_assignment`.
                    for (out_chunk_matching, _) in merge_join_chunks_by_key(&out_keys, &in_keys, |&key| key) {
                        debug_assert_eq!(out_chunk_matching.len(), 1);
                        push_support(out_chunk_matching[0], out_chunk_matching[0]);
                    }
                }

                if entries.len() == entries_len_before_search {
                    // Nothing was added: it means there no compatible groundings were found
                    entries.push((out_trans_id, in_trans_id, None));
                }
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

        Self { out, in_, entries }
    }

    pub fn iter_all(&self) -> impl Iterator<Item = &(TransitionId, TransitionId, Option<SupportGroundingId>)> {
        self.entries.iter()
    }
    pub fn iter_out_all(&self) -> impl Iterator<Item = &(TransitionId, SupportGroundingId, GroupEntry<TransitionId>)> {
        self.out.iter()
    }
    #[allow(clippy::type_complexity)]
    pub fn iter_in_all(
        &self,
    ) -> impl Iterator<Item = &(TransitionId, InGroupKey, GroupEntry<(TransitionId, SupportGroundingId)>)> {
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
