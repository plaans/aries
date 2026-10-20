mod utils;

use utils::{binary_search_range_by, merge_dedup_into};

use hashbrown::HashMap;

use aries_solver::core::IntCst;
use idmap::DirectIdMap;
use itertools::Itertools;

use crate::analysis::Source;
use crate::lprelax::transitions::supports::SupportsSorted;
use crate::lprelax::transitions::{TransitionId, Transitions};
use crate::{IntTerm, SchedEncoder};
use utils::merge_join_chunks_by_key;

pub type SourceGrounding = crate::analysis::grounding::ParametersAssignment;
pub type SourceGroundingId = usize;
pub type StateVarGroundingId = usize;

/// An id for transition groundings, consisting of:
/// the id of its ground state variable,
/// the value it reads (`val_assignment`, `None` for a pure effect),
/// and the value it hands over (`op_assignment`: the value written, or for a condition, the value read).
///
/// Sorts by state variable grounding id first.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransitionGroundingId {
    pub(super) state_var_grounding_id: StateVarGroundingId,
    pub(super) val_assignment: Option<IntCst>,
    pub(super) op_assignment: Option<IntCst>,
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

        // Intern each of the variable assignments in the source grounding,
        // marking each of them as appearing in it.
        for (i, &term) in source_terms.iter().enumerate() {
            let value = source_grounding[i];
            if !term.is_cst() {
                self.terms
                    .post_for_ground_source(term, value, source, source_grounding_id);
            }
        }

        // For each transition of the source, intern the corresponding grounding,
        // marking each of them as being appearing in the source grounding.
        // (Commented out: Also mark each of the variable assignments as appearing in the corresponding transition groundings.)
        for &trans_id in transitions.of_source(source) {
            let trans_grounding = transitions.evaluate_terms(trans_id, source_grounding, ctx);

            let trans_grounding_id = TransitionGroundingId {
                state_var_grounding_id: self.state_vars.post_ground_state_var(
                    &transitions.get_state_var(trans_id, ctx).fluent,
                    trans_grounding.args_evaluated(),
                ),
                val_assignment: trans_grounding.val_evaluated(),
                op_assignment: trans_grounding.op_evaluated(),
            };

            // for (term, value) in transitions.iter_evaluated_non_constant_terms(trans_id, source_grounding, ctx) {
            //     debug_assert!(!term.is_cst());
            //
            //     self.terms
            //         .post_for_ground_transition(term, value, trans_id, trans_grounding_id);
            // }

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

    /// Each distinct ground transition once (chunked by transition, but not necessarily sorted).
    pub fn iter_distinct_transition_groundings(&self) -> impl Iterator<Item = (TransitionId, TransitionGroundingId)> {
        self.transitions.iter_distinct()
    }

    /// For each source, the ground transitions with the source groundings they come from (sorted, so can be chunked).
    pub fn iter_transition_groundings_by_source(
        &self,
    ) -> impl Iterator<
        Item = (
            Source,
            impl Iterator<Item = &(TransitionId, TransitionGroundingId, SourceGroundingId)>,
        ),
    > {
        self.transitions.iter_by_source()
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
                    .take_while(|(trans_grounding_id, _)| trans_grounding_id.val_assignment.is_none())
                    .map(|&(_, trans_id)| trans_id)
                    .sorted_unstable()
                    .dedup()
                    .collect();
                (state_var_grounding_id, effects)
            })
    }

    /// The distinct ground state variables of the groundings of the transition (sorted).
    pub fn iter_ground_state_vars_of(&self, trans_id: TransitionId) -> impl Iterator<Item = StateVarGroundingId> {
        // (The groundings of a transition are sorted by ground state variable first.)
        self.transitions
            .groundings_of(trans_id)
            .iter()
            .map(|(_, trans_grounding_id, _)| trans_grounding_id.state_var_grounding_id)
            .dedup()
    }

    /// The ground supports of each (lifted) support, or `None` if it has none (sorted, so can be chunked).
    pub fn iter_support_groundings(
        &self,
    ) -> impl Iterator<Item = &(TransitionId, TransitionId, Option<SupportGroundingId>)> {
        self.supports.iter_all()
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

    /// The distinct assignments of (non-constant) terms to values (sorted, so can be chunked).
    pub fn iter_distinct_term_assignments(&self) -> impl Iterator<Item = (IntTerm, IntCst)> {
        self.terms.iter_distinct()
    }

    /// The assignments of (non-constant) terms to values, one entry per ground source in which they appear
    /// (sorted, so can be chunked).
    pub fn iter_term_assignments_with_sources(
        &self,
    ) -> impl Iterator<Item = &(IntTerm, IntCst, Source, SourceGroundingId)> {
        self.terms.iter_with_sources()
    }
}

#[derive(Clone, Default)]
struct SourcesGroundingsInfo {
    /// The (unique) groundings of the "empty source" (i.e. groundings corresponding to the "initial/final" action)
    entries_empty_source: Vec<SourceGroundingId>,
    /// The (unique) groundings of each "concrete source" (action/task)
    entries_concrete_sources: DirectIdMap<crate::TaskId, Vec<SourceGroundingId>>,
    /// Number of source groundings interned so far (i.e. id of the next one)
    num_groundings: usize,

    #[cfg(debug_assertions)]
    groundings: HashMap<(Source, SourceGrounding), SourceGroundingId>,
}
impl SourcesGroundingsInfo {
    /// Interns a grounding of a source.
    /// WARNING: adding duplicate groundings (for the same source) will result in a panic (in debug mode).
    ///
    /// Does not do anything relating to ground supports.
    fn post_ground_source(&mut self, source: Source, source_grounding: &SourceGrounding) -> SourceGroundingId {
        #[cfg(debug_assertions)]
        debug_assert!(!self.groundings.contains_key(&(source, source_grounding.clone())));

        let source_grounding_id = self.num_groundings;
        self.num_groundings += 1;

        #[cfg(debug_assertions)]
        {
            self.groundings
                .insert((source, source_grounding.clone()), source_grounding_id);
        }

        if let Some(task_id) = source {
            if !self.entries_concrete_sources.contains_key(task_id) {
                self.entries_concrete_sources.insert(task_id, vec![]);
            }
            self.entries_concrete_sources[task_id].push(source_grounding_id);
        } else {
            self.entries_empty_source.push(source_grounding_id);
        }
        source_grounding_id
    }

    fn get(&self, source: Source) -> &[SourceGroundingId] {
        if let Some(task_id) = source {
            debug_assert!(
                self.entries_concrete_sources
                    .get(task_id)
                    .is_none_or(|entries| entries.iter().all_unique())
            );
            self.entries_concrete_sources
                .get(task_id)
                .map(|entries| entries.as_slice())
                .unwrap_or_default()
        } else {
            debug_assert!(self.entries_empty_source.iter().all_unique());
            &self.entries_empty_source
        }
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
    /// Stores ids of state variable groundings (they are used in transition grounding ids as the first element of the triple)
    entries: HashMap<(FluentId, StateVarGrounding), StateVarGroundingId>,

    #[cfg(debug_assertions)]
    groundings: Vec<(FluentId, StateVarGrounding)>,
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
                debug_assert!(state_var_grounding_id == self.groundings.len());
                self.groundings.push((fluent_id, state_var_grounding.into()));
            }
            state_var_grounding_id
        }
    }
}

#[derive(Clone, Default)]
struct TermsGroundingsInfo {
    // /// Flat storage of term assignments with the ground transitions in which they appear.
    // entries_merged_transitions: Vec<(IntTerm, IntCst, TransitionId, TransitionGroundingId)>,
    /// Flat storage of term assignments with the ground sources in which they appear.
    entries_merged_sources: Vec<(IntTerm, IntCst, Source, SourceGroundingId)>,

    // /// Same as for sources.
    // entries_pending_transitions: Vec<(IntTerm, IntCst, TransitionId, TransitionGroundingId)>,
    /// A buffer into which the entries are pushed before being deduped, sorted, and merged into the
    /// corresponding "merged" field (see above [`entries_merged_sources`]).
    /// This could allow a more efficient incremental interning of new entries,
    /// via interleaved sorting then merging, rather than global merging every time.
    entries_pending_sources: Vec<(IntTerm, IntCst, Source, SourceGroundingId)>,
}
impl TermsGroundingsInfo {
    // /// Records that a (non-constant) term is assigned a value in a ground transition.
    // pub fn post_for_ground_transition(
    //     &mut self,
    //     term: IntTerm,
    //     value: IntCst,
    //     trans_id: TransitionId,
    //     trans_grounding_id: TransitionGroundingId,
    // ) {
    //     assert!(!term.is_cst());
    //     debug_assert!(
    //         !self
    //             .entries_merged_transitions
    //             .contains(&(term, value, trans_id, trans_grounding_id))
    //     );
    //
    //     self.entries_pending_transitions
    //         .push((term, value, trans_id, trans_grounding_id));
    // }

    /// Records that a (non-constant) term is assigned a value in a ground source.
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

    // fn cmp_trs(
    //     x: &(IntTerm, IntCst, TransitionId, TransitionGroundingId),
    //     y: &(IntTerm, IntCst, TransitionId, TransitionGroundingId),
    // ) -> std::cmp::Ordering {
    //     x.cmp(y)
    // }
    fn cmp_src(
        x: &(IntTerm, IntCst, Source, SourceGroundingId),
        y: &(IntTerm, IntCst, Source, SourceGroundingId),
    ) -> std::cmp::Ordering {
        x.cmp(y)
    }

    fn sort(&mut self) {
        // self.sort_and_merge_pending_transitions();
        self.sort_and_merge_pending_sources();
    }

    // fn sort_and_merge_pending_transitions(&mut self) {
    //     self.entries_pending_transitions.sort_unstable();
    //     self.entries_pending_transitions.dedup();
    //     merge_dedup_into(
    //         &mut self.entries_pending_transitions,
    //         &mut self.entries_merged_transitions,
    //         Self::cmp_trs,
    //     );
    //     debug_assert!(self.entries_pending_transitions.is_empty());
    //     debug_assert!(self.entries_merged_transitions.is_sorted());
    // }
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

    // fn debug_check_valid_for_transitions(&self) -> bool {
    //     debug_assert!(self.entries_pending_transitions.is_empty());
    //     debug_assert!(is_sorted_and_no_dupes(self.entries_merged_transitions.iter()));
    //     true
    // }
    fn debug_check_valid_for_sources(&self) -> bool {
        debug_assert!(self.entries_pending_sources.is_empty());
        debug_assert!(is_sorted_and_no_dupes(self.entries_merged_sources.iter()));
        true
    }

    // /// Sorted, meaning the iterator can be chunked (without extra allocations).
    // pub fn iter_sorted_all_for_transitions(
    //     &self,
    // ) -> impl Iterator<Item = &(IntTerm, IntCst, TransitionId, TransitionGroundingId)> {
    //     debug_assert!(self.debug_check_valid_for_transitions());
    //     self.entries_merged_transitions.iter()
    // }

    /// The assignments of (non-constant) terms to values, one entry per ground source in which they appear.
    /// Sorted, so can be chunked
    fn iter_with_sources(&self) -> impl Iterator<Item = &(IntTerm, IntCst, Source, SourceGroundingId)> {
        debug_assert!(self.debug_check_valid_for_sources());
        self.entries_merged_sources.iter()
    }

    /// The distinct assignments of (non-constant) terms to values.
    /// Sorted, so can be chunked.
    fn iter_distinct(&self) -> impl Iterator<Item = (IntTerm, IntCst)> {
        debug_assert!(self.debug_check_valid_for_sources());
        self.entries_merged_sources
            .iter()
            .map(|(term, value, _, _)| (*term, *value))
            .dedup()
    }
}

#[derive(Clone, Default)]
struct TransitionsGroundingsInfo {
    sources_of: DirectIdMap<TransitionId, Source>,

    /// The groundings of the empty source's transitions, one entry per source grounding they come from.
    /// Sorted, so can be chunked.
    entries_empty_source: Vec<(TransitionId, TransitionGroundingId, SourceGroundingId)>,
    /// The groundings of the each concrete source's transitions, one entry per source grounding they come from.
    /// Sorted, so can be chunked.
    /// Different ground sources may contain the same ground transition (if they only differ on parameters the transition doesn't use).
    entries_concrete_sources: DirectIdMap<crate::TaskId, Vec<(TransitionId, TransitionGroundingId, SourceGroundingId)>>,

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

        if let Some(task_id) = source {
            if !self.entries_concrete_sources.contains_key(task_id) {
                self.entries_concrete_sources.insert(task_id, vec![]);
            }
            debug_assert!(!self.entries_concrete_sources[task_id].contains(&(
                trans_id,
                trans_grounding_id,
                source_grounding_id
            )));
            self.entries_concrete_sources[task_id].push((trans_id, trans_grounding_id, source_grounding_id));
        } else {
            debug_assert!(
                !self
                    .entries_empty_source
                    .contains(&(trans_id, trans_grounding_id, source_grounding_id))
            );
            self.entries_empty_source
                .push((trans_id, trans_grounding_id, source_grounding_id));
        }
    }

    fn debug_check_valid(&self) -> bool {
        debug_assert!(is_sorted_and_no_dupes(self.entries_empty_source.iter()));
        debug_assert!(is_sorted_and_no_dupes(self.entries_concrete_sources.iter()));
        true
    }

    /// Sorts the groundings, then indexes them by ground state variable (see [`Self::iter_by_state_var`]).
    fn sort_for_all(&mut self) {
        self.entries_empty_source.sort_unstable();
        for (_, entries) in self.entries_concrete_sources.iter_mut() {
            entries.sort_unstable();
        }

        self.entries_by_state_var = self
            .iter_distinct()
            .map(|(trans_id, trans_grounding_id)| (trans_grounding_id, trans_id))
            .sorted_unstable()
            .collect();
        debug_assert!(is_sorted_and_no_dupes(self.entries_by_state_var.iter()));
    }

    /// The distinct ground transitions of each ground state variable.
    /// They are sorted -- as [`TransitionGroundingId`] is (by `val_assignment`, then `op_assignment`) -- so they can be chunked.
    fn iter_by_state_var(
        &self,
    ) -> impl Iterator<Item = (StateVarGroundingId, &[(TransitionGroundingId, TransitionId)])> {
        self.entries_by_state_var
            .chunk_by(|a, b| a.0.state_var_grounding_id == b.0.state_var_grounding_id)
            .map(|chunk| (chunk[0].0.state_var_grounding_id, chunk))
    }

    /// Each ground transition with the source groundings it comes from.
    /// Sorted, so can be chunked.
    fn iter_by_source(
        &self,
    ) -> impl Iterator<
        Item = (
            Source,
            impl Iterator<Item = &(TransitionId, TransitionGroundingId, SourceGroundingId)>,
        ),
    > {
        debug_assert!(self.debug_check_valid());
        std::iter::chain(
            [(None, self.entries_empty_source.iter())],
            self.entries_concrete_sources
                .iter()
                .map(|(source, entries)| (Some(source), entries.iter())),
        )
    }

    /// Each distinct ground transition once (i.e. without the source groundings it comes from).
    ///
    /// WARNING: although the iterator can be chunked (without extra allocations) by transition, it may not necessarily be *sorted* !!
    fn iter_distinct(&self) -> impl Iterator<Item = (TransitionId, TransitionGroundingId)> {
        debug_assert!(self.debug_check_valid());
        // result is not necessarily sorted, but chunked because a transition is "owned" only by a single source
        std::iter::chain(
            self.entries_empty_source
                .iter()
                .map(|(trans_id, trans_grounding_id, _)| (*trans_id, *trans_grounding_id)),
            self.entries_concrete_sources.iter().flat_map(|(_, entries)| {
                entries
                    .iter()
                    .map(|(trans_id, trans_grounding_id, _)| (*trans_id, *trans_grounding_id))
            }),
        )
        .dedup()
    }

    /// The groundings of the given transition (sorted, so they can be chunked).
    fn groundings_of(&self, trans_id: TransitionId) -> &[(TransitionId, TransitionGroundingId, SourceGroundingId)] {
        debug_assert!(self.debug_check_valid());

        let Some(&source) = self.sources_of.get(trans_id) else {
            return [].as_slice();
        };
        let entries = if let Some(task_id) = source {
            &self.entries_concrete_sources[task_id]
        } else {
            &self.entries_empty_source
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
/// The carried / handed over value is that of the out-transition's `op_assignment`.
pub type SupportGroundingId = (StateVarGroundingId, IntCst);

/// A group of groundings of an in-transition that can receive the same ground supports:
/// those sharing a state variable grounding and a `val_assignment` (whatever their `op_assignment`).
///
/// A ground support into the group hands over that `val_assignment` value, or any value if it is `None` (pure effect).
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
    /// Grounds the given (lifted) supports, and stores them in three views (see fields).
    ///
    /// A lifted support gives a ground support for each pair (state variable grounding id, value) that a
    /// grounding of its out-transition hands over and that a grounding of its in-transition can receive (see [`SupportGroundingId`] and [`InGroupKey`]).
    /// If there is no such ground support, the "neutral" view ([`Self::entries`]) is to contain a `None` element instead.
    ///
    /// The incoming view ([`Self::in_`]) groups the groundings of every transition, even if nothing can support them
    ///
    /// Expects the transitions groundings to be sorted (see [`TransitionsGroundingsInfo::sort_for_all`]).
    fn from(supports_sorted: &SupportsSorted, transitions_groundings: &TransitionsGroundingsInfo) -> Self {
        debug_assert!(transitions_groundings.debug_check_valid());

        // The groundings of all the transitions, grouped by the ground supports they can receive.
        // A group that nothing can support is left with only these entries.
        let mut in_ = transitions_groundings
            .iter_distinct()
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
                    .groundings_of(out_trans_id)
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

    fn iter_all(&self) -> impl Iterator<Item = &(TransitionId, TransitionId, Option<SupportGroundingId>)> {
        self.entries.iter()
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

        assert_eq!(sources_groundings.entries_empty_source, vec![3]);
        assert!(
            sources_groundings.entries_concrete_sources == {
                let mut res = DirectIdMap::new();
                res.insert(TaskId::from_int(0), vec![2, 4, 5, 6]);
                res.insert(TaskId::from_int(1), vec![0, 1]);
                res
            }
        );
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
