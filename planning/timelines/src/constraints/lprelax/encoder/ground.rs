mod utils;

use utils::binary_search_range_by;

use hashbrown::HashMap;

use aries_solver::core::IntCst;
use idmap::DirectIdMap;
use itertools::Itertools;

use crate::analysis::Source;
use crate::lprelax::BySource;
use crate::lprelax::transitions::supports::SupportsSorted;
use crate::lprelax::transitions::{TransitionId, Transitions};
use crate::{IntTerm, SchedEncoder};
use utils::merge_join_chunks_by_key;

pub type SourceGrounding = crate::analysis::grounding::ParametersAssignment;
pub type SourceGroundingId = usize;
pub type StateVarGroundingId = usize;

/// The id of a transition grounding: its ground state variable (by which it sorts first),
/// the value it reads (`pre_val_assignment`, `None` for an eff),
/// and the value it hands over (`post_op_assignment`).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransitionGroundingId {
    pub(super) state_var_grounding_id: StateVarGroundingId,
    pub(super) pre_val_assignment: Option<IntCst>,
    pub(super) post_op_assignment: Option<IntCst>,
}

#[derive(Clone, Default)]
pub(super) struct GroundingsInfo {
    sources: SourcesGroundingsInfo,
    state_vars: StateVarsGroundingsInfo,
    transitions: TransitionsGroundingsInfo,
    terms: TermsGroundingsInfo,
    supports: SupportsGroundingsInfo,
}
impl GroundingsInfo {
    /// Interns a grounding of a source, and those of its terms and transitions.
    /// Will panic (in debug mode) if the ground source has already been interned.
    pub fn post_ground_source(
        &mut self,
        source: Source,
        source_terms: &[IntTerm],
        source_grounding: &SourceGrounding,
        transitions: &Transitions,
        ctx: &SchedEncoder,
    ) {
        let source_grounding_id = self.sources.post_ground_source(source, source_grounding);

        // Intern the assignments of the source's (non-constant) terms.
        for (i, &term) in source_terms.iter().enumerate() {
            let value = source_grounding[i];
            if !term.is_cst() {
                self.terms
                    .post_for_ground_source(term, value, source, source_grounding_id);
            }
        }

        // Intern the groundings of the source's transitions.
        for &trans_id in transitions.of_source(source) {
            let trans_grounding = transitions.evaluate_terms(trans_id, source_grounding, ctx);

            let trans_grounding_id = TransitionGroundingId {
                state_var_grounding_id: self.state_vars.post_ground_state_var(
                    &transitions.get_state_var(trans_id, ctx).fluent,
                    trans_grounding.state_var_args_eval(),
                ),
                pre_val_assignment: trans_grounding.pre_val_eval(),
                post_op_assignment: trans_grounding.post_op_eval(),
            };

            self.transitions
                .post_ground_transition(trans_id, trans_grounding_id, source, source_grounding_id);
        }
    }

    /// Sorts the groundings, then grounds the given (lifted) supports.
    pub fn sort(&mut self, supports_sorted: &SupportsSorted) {
        let mut times = vec![];

        let time = std::time::Instant::now();
        self.transitions.sort_for_all();
        times.push(time.elapsed());

        let time = std::time::Instant::now();
        self.supports = SupportsGroundingsInfo::from(supports_sorted, &self.transitions);
        times.push(time.elapsed());

        let time = std::time::Instant::now();
        self.terms.sort();
        times.push(time.elapsed());

        lprelax_log!(
            "Sorted groundings in {} (transitions groundings {}, [built and sorted] support groundings {}, terms groundings {})",
            times.iter().sum::<std::time::Duration>().as_secs_f64(),
            times[0].as_secs_f64(),
            times[1].as_secs_f64(),
            times[2].as_secs_f64(),
        );
    }

    /// The groundings of the given source.
    pub fn source_groundings(&self, source: Source) -> &[SourceGroundingId] {
        self.sources.get(source)
    }

    /// For each transition, its distinct groundings.
    pub fn iter_transition_groundings(
        &self,
    ) -> impl Iterator<Item = (TransitionId, impl Iterator<Item = TransitionGroundingId>)> {
        self.transitions.iter_by_transition()
    }

    /// For each ground transition, its source and this source's compatible groundings.
    pub fn iter_transition_groundings_with_sources(
        &self,
    ) -> impl Iterator<
        Item = (
            TransitionId,
            TransitionGroundingId,
            Source,
            impl ExactSizeIterator<Item = SourceGroundingId>,
        ),
    > {
        self.transitions.iter_with_sources()
    }

    /// The distinct ground transitions of each ground state variable (sorted, so can be chunked).
    pub fn iter_transition_groundings_by_state_var(
        &self,
    ) -> impl Iterator<Item = (StateVarGroundingId, &[(TransitionGroundingId, TransitionId)])> {
        self.transitions.iter_by_state_var()
    }

    /// The distinct effects of each ground state variable (sorted).
    pub fn iter_effects_by_state_var(&self) -> impl Iterator<Item = (StateVarGroundingId, Vec<TransitionId>)> {
        self.iter_transition_groundings_by_state_var()
            .map(|(state_var_grounding_id, entries)| {
                // (The effects come first, as they read no value.)
                let effects = entries
                    .iter()
                    .take_while(|(trans_grounding_id, _)| trans_grounding_id.pre_val_assignment.is_none())
                    .map(|&(_, trans_id)| trans_id)
                    .sorted_unstable()
                    .dedup()
                    .collect();
                (state_var_grounding_id, effects)
            })
    }

    #[allow(dead_code)]
    /// The distinct ground state variables of the groundings of the transition (sorted).
    pub fn iter_ground_state_vars_of(&self, trans_id: TransitionId) -> impl Iterator<Item = StateVarGroundingId> {
        // (The groundings of a transition are sorted by ground state variable first.)
        self.transitions
            .groundings_of(trans_id)
            .iter()
            .map(|(_, trans_grounding_id, _)| trans_grounding_id.state_var_grounding_id)
            .dedup()
    }

    /// For each (lifted) support, its ground supports (possibly none).
    pub fn iter_support_groundings(
        &self,
    ) -> impl Iterator<Item = (TransitionId, TransitionId, impl Iterator<Item = SupportGroundingId>)> {
        self.supports.iter_by_support()
    }

    /// The groups of the outgoing view of the ground supports (see [`SupportsGroundingsInfo::iter_out_groups`]).
    pub fn iter_support_out_groups(
        &self,
    ) -> impl Iterator<
        Item = (
            TransitionId,
            SupportGroundingId,
            impl ExactSizeIterator<Item = TransitionGroundingId>,
            impl ExactSizeIterator<Item = TransitionId>,
        ),
    > {
        self.supports.iter_out_groups()
    }

    /// The groups of the incoming view of the ground supports (see [`SupportsGroundingsInfo::iter_in_groups`]).
    pub fn iter_support_in_groups(
        &self,
    ) -> impl Iterator<
        Item = (
            TransitionId,
            InGroupKey,
            impl ExactSizeIterator<Item = TransitionGroundingId>,
            impl ExactSizeIterator<Item = (TransitionId, SupportGroundingId)>,
        ),
    > {
        self.supports.iter_in_groups()
    }

    /// The distinct assignments of (non-constant) terms to values.
    pub fn iter_distinct_term_assignments(&self) -> impl Iterator<Item = (IntTerm, IntCst)> {
        self.terms.iter_distinct()
    }

    /// For each (non-constant) term, its distinct values.
    pub fn iter_term_values(&self) -> impl Iterator<Item = (IntTerm, impl Iterator<Item = IntCst>)> {
        self.terms.iter_values()
    }

    /// For each assignment of a (non-constant) term to a value, and each source using the term,
    /// the groundings of the source with that assignment.
    pub fn iter_term_assignments_with_sources(
        &self,
    ) -> impl Iterator<
        Item = (
            IntTerm,
            IntCst,
            Source,
            impl ExactSizeIterator<Item = SourceGroundingId>,
        ),
    > {
        self.terms.iter_with_sources()
    }
}

#[derive(Clone, Default)]
struct SourcesGroundingsInfo {
    /// The (unique) groundings of each source.
    entries: BySource<Vec<SourceGroundingId>>,
    /// Number of source groundings interned so far (i.e. id of the next one)
    num_groundings: usize,

    #[cfg(debug_assertions)]
    groundings: HashMap<(Source, SourceGrounding), SourceGroundingId>,
}
impl SourcesGroundingsInfo {
    /// Interns a grounding of a source (which must be new: checked in debug mode).
    fn post_ground_source(&mut self, source: Source, _source_grounding: &SourceGrounding) -> SourceGroundingId {
        #[cfg(debug_assertions)]
        debug_assert!(!self.groundings.contains_key(&(source, _source_grounding.clone())));

        let source_grounding_id = self.num_groundings;
        self.num_groundings += 1;

        #[cfg(debug_assertions)]
        {
            self.groundings
                .insert((source, _source_grounding.clone()), source_grounding_id);
        }

        self.entries.get_mut_or_default(source).push(source_grounding_id);
        source_grounding_id
    }

    fn get(&self, source: Source) -> &[SourceGroundingId] {
        let entries = self
            .entries
            .get(source)
            .map(|entries| entries.as_slice())
            .unwrap_or_default();
        debug_assert!(entries.iter().all_unique());
        entries
    }
}

type FluentId = usize;
type StateVarGrounding = smallvec::SmallVec<[IntCst; 4]>;

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
struct StateVarsGroundingsInfo {
    fluents_ids: HashMap<crate::Sym, FluentId>,
    /// The ids of the ground state variables.
    entries: HashMap<(FluentId, StateVarGrounding), StateVarGroundingId>,

    #[cfg(debug_assertions)]
    groundings: Vec<(FluentId, StateVarGrounding)>,
}
impl StateVarsGroundingsInfo {
    /// The id of the given ground state variable, interning it if new.
    fn post_ground_state_var(&mut self, fluent: &crate::Sym, state_var_grounding: &[IntCst]) -> StateVarGroundingId {
        let num_fluents = self.fluents_ids.len();
        let fluent_id = *self.fluents_ids.entry_ref(fluent).or_insert(num_fluents);

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
                debug_assert!(state_var_grounding_id == self.groundings.len());
                self.groundings.push((fluent_id, state_var_grounding.into()));
            }
            state_var_grounding_id
        }
    }
}

#[derive(Clone, Default)]
struct TermsGroundingsInfo {
    /// The assignments of (non-constant) terms to values, with the ground sources in which they appear.
    /// Sorted (and deduplicated) once all are posted.
    entries: Vec<(IntTerm, IntCst, Source, SourceGroundingId)>,
}
impl TermsGroundingsInfo {
    /// Records that a (non-constant) term is assigned a value in a ground source.
    fn post_for_ground_source(
        &mut self,
        term: IntTerm,
        value: IntCst,
        source: Source,
        source_grounding_id: SourceGroundingId,
    ) {
        assert!(!term.is_cst());
        self.entries.push((term, value, source, source_grounding_id));
    }

    fn sort(&mut self) {
        self.entries.sort_unstable();
        self.entries.dedup();
    }

    /// Whether the entries are sorted, without duplicates (see [`Self::sort`]).
    fn is_sorted(&self) -> bool {
        is_sorted_and_no_dupes(self.entries.iter())
    }

    /// The distinct assignments of (non-constant) terms to values.
    fn iter_distinct(&self) -> impl Iterator<Item = (IntTerm, IntCst)> {
        debug_assert!(self.is_sorted());
        self.entries.iter().map(|&(term, value, _, _)| (term, value)).dedup()
    }

    /// For each (non-constant) term, its distinct values.
    fn iter_values(&self) -> impl Iterator<Item = (IntTerm, impl Iterator<Item = IntCst>)> {
        debug_assert!(self.is_sorted());
        self.entries
            .chunk_by(|a, b| a.0 == b.0)
            .map(|chunk| (chunk[0].0, chunk.iter().map(|&(_, value, _, _)| value).dedup()))
    }

    /// For each assignment of a (non-constant) term to a value, and each source using the term,
    /// the groundings of the source with that assignment.
    fn iter_with_sources(
        &self,
    ) -> impl Iterator<
        Item = (
            IntTerm,
            IntCst,
            Source,
            impl ExactSizeIterator<Item = SourceGroundingId>,
        ),
    > {
        debug_assert!(self.is_sorted());
        self.entries
            .chunk_by(|a, b| (a.0, a.1, a.2) == (b.0, b.1, b.2))
            .map(|chunk| {
                let (term, value, source, _) = chunk[0];
                (term, value, source, chunk.iter().map(|&(_, _, _, g)| g))
            })
    }
}

#[derive(Clone, Default)]
struct TransitionsGroundingsInfo {
    sources_of: DirectIdMap<TransitionId, Source>,

    /// The groundings of each source's transitions, one entry per source grounding they come from (sorted).
    entries: BySource<Vec<(TransitionId, TransitionGroundingId, SourceGroundingId)>>,

    /// The distinct groundings of all transitions (one entry each, whatever source groundings they come from).
    /// Sorted, so by ground state variable first (see [`Self::iter_by_state_var`]).
    entries_by_state_var: Vec<(TransitionGroundingId, TransitionId)>,
}
impl TransitionsGroundingsInfo {
    fn post_ground_transition(
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

        // (Duplicates are caught by `is_sorted`, once sorted.)
        let entries = self.entries.get_mut_or_default(source);
        entries.push((trans_id, trans_grounding_id, source_grounding_id));
    }

    /// Whether each source's entries are sorted, without duplicates (see [`Self::sort_for_all`]).
    fn is_sorted(&self) -> bool {
        self.entries
            .iter()
            .all(|(_, entries)| is_sorted_and_no_dupes(entries.iter()))
    }

    /// Sorts the groundings, then indexes them by ground state variable (see [`Self::iter_by_state_var`]).
    fn sort_for_all(&mut self) {
        for entries in self.entries.values_mut() {
            entries.sort_unstable();
        }

        self.entries_by_state_var = self
            .iter_distinct()
            .map(|(trans_id, trans_grounding_id)| (trans_grounding_id, trans_id))
            .sorted_unstable()
            .collect();
        debug_assert!(is_sorted_and_no_dupes(self.entries_by_state_var.iter()));
    }

    /// The distinct ground transitions of each ground state variable
    /// (sorted, as [`TransitionGroundingId`] is: by `pre_val_assignment`, then `post_op_assignment`).
    fn iter_by_state_var(
        &self,
    ) -> impl Iterator<Item = (StateVarGroundingId, &[(TransitionGroundingId, TransitionId)])> {
        self.entries_by_state_var
            .chunk_by(|a, b| a.0.state_var_grounding_id == b.0.state_var_grounding_id)
            .map(|chunk| (chunk[0].0.state_var_grounding_id, chunk))
    }

    /// The (sorted) groundings of each source's transitions, empty source first.
    fn iter_entries_by_source(
        &self,
    ) -> impl Iterator<Item = (Source, &[(TransitionId, TransitionGroundingId, SourceGroundingId)])> {
        debug_assert!(self.is_sorted());
        self.entries
            .iter()
            .map(|(source, entries)| (source, entries.as_slice()))
    }

    /// For each transition, its distinct groundings.
    fn iter_by_transition(&self) -> impl Iterator<Item = (TransitionId, impl Iterator<Item = TransitionGroundingId>)> {
        // (A transition belongs to a single source.)
        self.iter_entries_by_source().flat_map(|(_, entries)| {
            entries
                .chunk_by(|a, b| a.0 == b.0)
                .map(|chunk| (chunk[0].0, chunk.iter().map(|&(_, g, _)| g).dedup()))
        })
    }

    /// For each ground transition, its source and the groundings of its source it comes from.
    fn iter_with_sources(
        &self,
    ) -> impl Iterator<
        Item = (
            TransitionId,
            TransitionGroundingId,
            Source,
            impl ExactSizeIterator<Item = SourceGroundingId>,
        ),
    > {
        self.iter_entries_by_source().flat_map(|(source, entries)| {
            entries
                .chunk_by(|a, b| (a.0, a.1) == (b.0, b.1))
                .map(move |chunk| (chunk[0].0, chunk[0].1, source, chunk.iter().map(|&(_, _, g)| g)))
        })
    }

    /// Each distinct ground transition once (grouped by transition, but not sorted).
    fn iter_distinct(&self) -> impl Iterator<Item = (TransitionId, TransitionGroundingId)> {
        self.iter_by_transition()
            .flat_map(|(trans_id, groundings)| groundings.map(move |g| (trans_id, g)))
    }

    /// The groundings of the given transition (sorted, so they can be chunked).
    fn groundings_of(&self, trans_id: TransitionId) -> &[(TransitionId, TransitionGroundingId, SourceGroundingId)] {
        debug_assert!(self.is_sorted());

        let Some(entries) = self
            .sources_of
            .get(trans_id)
            .and_then(|&source| self.entries.get(source))
        else {
            return [].as_slice();
        };
        binary_search_range_by(entries, |(trans_id_, _, _)| (*trans_id_).cmp(&trans_id))
            .map_or_else(|| [].as_slice(), |(i, j)| &entries[i..j])
    }
}

/// A ground support (between two transitions): the ground state variable, and the value handed over
/// (the `post_op_assignment` of the out-transition's groundings).
pub type SupportGroundingId = (StateVarGroundingId, IntCst);

/// The groundings of an in-transition that receive the same ground supports:
/// those with this ground state variable and `pre_val_assignment` (`None`, for an eff, receives any value).
pub type InGroupKey = (StateVarGroundingId, Option<IntCst>);

/// An entry of a group of groundings (of a same transition) in the outgoing / incoming views of [`SupportsGroundingsInfo`].
/// Within a group, the groundings come first, then the ground supports.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum GroupEntry<S> {
    /// One of the (distinct) groundings of the group.
    Grounding(TransitionGroundingId),
    /// A ground support out of / into the group.
    Support(S),
}

#[derive(Clone, Default)]
struct SupportsGroundingsInfo {
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
    /// Grounds the given (lifted) supports into the three views (see the fields).
    ///
    /// A lifted support gets a ground support for each (ground state variable, value) that its out-transition's groundings hand over
    /// and its in-transition's groundings can receive, or else a `None` entry in [`Self::entries`].
    /// The incoming view also holds the groundings that nothing supports.
    ///
    /// Expects the transitions groundings to be sorted.
    fn from(supports_sorted: &SupportsSorted, transitions_groundings: &TransitionsGroundingsInfo) -> Self {
        debug_assert!(transitions_groundings.is_sorted());

        // The groundings of all the transitions, grouped by the ground supports they can receive.
        // A group that nothing can support is left with only these entries.
        let mut in_ = transitions_groundings
            .iter_distinct()
            .map(|(trans_id, trans_grounding_id)| {
                (
                    trans_id,
                    (
                        trans_grounding_id.state_var_grounding_id,
                        trans_grounding_id.pre_val_assignment,
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
        for (out_trans_id, in_trans_ids) in supports_sorted.iter_by_out() {
            // The groundings of the out-transition, grouped by the ground support they can hand over.
            let out_len_before = out.len();
            out.extend(
                transitions_groundings
                    .groundings_of(out_trans_id)
                    .iter()
                    .map(|&(_, trans_grounding_id, _)| {
                        let support_grounding_id: SupportGroundingId = (
                            trans_grounding_id.state_var_grounding_id,
                            trans_grounding_id.post_op_assignment.unwrap(),
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

            for (in_trans_id, _) in in_trans_ids {
                let in_keys: Vec<InGroupKey> =
                    binary_search_range_by(&in_groups, |(trans_id, _)| trans_id.cmp(&in_trans_id))
                        .map_or(&[][..], |(i, j)| &in_groups[i..j])
                        .iter()
                        .map(|&(_, in_group_key)| in_group_key)
                        .collect::<Vec<_>>();

                let entries_len_before_search = entries.len();

                for_each_ground_support(&out_keys, &in_keys, |support_grounding_id, in_group_key| {
                    out.push((out_trans_id, support_grounding_id, GroupEntry::Support(in_trans_id)));
                    in_.push((
                        in_trans_id,
                        in_group_key,
                        GroupEntry::Support((out_trans_id, support_grounding_id)),
                    ));
                    entries.push((out_trans_id, in_trans_id, Some(support_grounding_id)));
                });

                if entries.len() == entries_len_before_search {
                    // No compatible groundings
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

    /// For each (lifted) support, its ground supports (possibly none).
    fn iter_by_support(
        &self,
    ) -> impl Iterator<Item = (TransitionId, TransitionId, impl Iterator<Item = SupportGroundingId>)> {
        self.entries
            .chunk_by(|a, b| (a.0, a.1) == (b.0, b.1))
            .map(|chunk| (chunk[0].0, chunk[0].1, chunk.iter().filter_map(|&(_, _, g)| g)))
    }
    /// The groups of the outgoing view:
    /// (
    ///   out_trans_id,
    ///   support_grounding_id,
    ///   groundings of out_trans_id handing over the ground support's value on the ground support's ground state variable,
    ///   in_trans_ids these groundings can support
    /// ).
    fn iter_out_groups(
        &self,
    ) -> impl Iterator<
        Item = (
            TransitionId,
            SupportGroundingId,
            impl ExactSizeIterator<Item = TransitionGroundingId>,
            impl ExactSizeIterator<Item = TransitionId>,
        ),
    > {
        iter_support_groups(&self.out)
    }
    /// The groups of the incoming view:
    /// (
    ///   in_trans_id,
    ///   in_group_key,
    ///   groundings of in_trans_id for that key,
    ///   ground supports (out_trans_id, support_grounding_id) that these groundings can receive
    /// ).
    fn iter_in_groups(
        &self,
    ) -> impl Iterator<
        Item = (
            TransitionId,
            InGroupKey,
            impl ExactSizeIterator<Item = TransitionGroundingId>,
            impl ExactSizeIterator<Item = (TransitionId, SupportGroundingId)>,
        ),
    > {
        iter_support_groups(&self.in_)
    }
}

/// Calls `f` on each ground support from an out-transition to an in-transition, with the group of the in-transition receiving it,
/// given the (sorted) keys of the values the out-transition hands over and the (sorted) groups of the in-transition.
fn for_each_ground_support(
    out_keys: &[InGroupKey],
    in_keys: &[InGroupKey],
    mut f: impl FnMut(SupportGroundingId, InGroupKey),
) {
    let in_is_eff = in_keys.first().is_some_and(|&(_, pre_val)| pre_val.is_none());
    if in_is_eff {
        // An eff needs no value: any value handed over on its ground state variable supports it.
        for (outs, ins) in
            merge_join_chunks_by_key(out_keys, in_keys, |&(state_var_grounding_id, _)| state_var_grounding_id)
        {
            debug_assert_eq!(ins.len(), 1);
            for &(state_var_grounding_id, value) in outs {
                f((state_var_grounding_id, value.unwrap()), ins[0]);
            }
        }
    } else {
        // Any other in-transition needs the value handed over to be its `pre_val_assignment`.
        for (outs, _) in merge_join_chunks_by_key(out_keys, in_keys, |&key| key) {
            debug_assert_eq!(outs.len(), 1);
            let (state_var_grounding_id, value) = outs[0];
            f((state_var_grounding_id, value.unwrap()), outs[0]);
        }
    }
}

/// Splits a (sorted) view of [`SupportsGroundingsInfo`] into its groups:
/// (trans_id, key, groundings of trans_id, ground supports out of / into them).
fn iter_support_groups<K: Copy + PartialEq, S: Copy>(
    entries: &[(TransitionId, K, GroupEntry<S>)],
) -> impl Iterator<
    Item = (
        TransitionId,
        K,
        impl ExactSizeIterator<Item = TransitionGroundingId>,
        impl ExactSizeIterator<Item = S>,
    ),
> {
    entries.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)).map(|group| {
        // Within a group, the groundings come first.
        let num_groundings = group.partition_point(|(_, _, entry)| matches!(entry, GroupEntry::Grounding(_)));
        let (groundings, supports) = group.split_at(num_groundings);
        (
            group[0].0,
            group[0].1,
            groundings.iter().map(|&(_, _, entry)| match entry {
                GroupEntry::Grounding(trans_grounding_id) => trans_grounding_id,
                GroupEntry::Support(_) => unreachable!(),
            }),
            supports.iter().map(|&(_, _, entry)| match entry {
                GroupEntry::Support(support) => support,
                GroupEntry::Grounding(_) => unreachable!(),
            }),
        )
    })
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
    fn test_ground_sources_posting() {
        let mut sources_groundings = SourcesGroundingsInfo::default();

        sources_groundings.post_ground_source(Some(TaskId::from_int(1)), &SourceGrounding::from(vec![1000, 50, 40]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(1)), &SourceGrounding::from(vec![1000, 50, 30]));

        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![2, 10, 400, 300]));

        sources_groundings.post_ground_source(None, &SourceGrounding::from(vec![]));

        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![2, 10, 401, 300]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![2, 10, 400, 301]));
        sources_groundings.post_ground_source(Some(TaskId::from_int(0)), &SourceGrounding::from(vec![2, 10, 401, 301]));

        assert_eq!(sources_groundings.get(None), &[3]);
        assert_eq!(sources_groundings.get(Some(TaskId::from_int(0))), &[2, 4, 5, 6]);
        assert_eq!(sources_groundings.get(Some(TaskId::from_int(1))), &[0, 1]);
    }

    #[test]
    fn test_ground_sources_posting_panic_on_dupe() {
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
    fn test_ground_state_vars_posting() {
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
}
