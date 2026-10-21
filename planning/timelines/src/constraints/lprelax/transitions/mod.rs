//! NOTE: Although this module is contained in the [`lprelax`] module -- since that's the only one using it -- it is completely independent of [`lprelax`].
//!
//! This module allows defines "transitions", which can be:
//! - cond-transitions (or simply conditions),
//! - eff-transitions (or simply effects),
//! - condeff-transitions (or pure transitions), i.e. pairs (condition, effect) where the condition and effect share:
//!   - their source (container action instance),
//!   - presence literal,
//!   - their state variable name (state function),
//!   - and their state variable parameter-variables (arguments).
//!
//! The module allows collecting and manipulating transitions. More specifically:
//! - The "unambiguous" (i.e. non-"nonsimple") transitions in a scheduling problem can be collected into the [`Transitions`] structure.
//! - Some helper functions are provided, for example to allow evaluating a transition's terms against a
//!   grounding of its source (i.e. an assignment to its container action's parameters).
//! - Potential lifted support links between transitions can be collected into the [`supports::Supports`] structure.
//!   In addition to the effect-to-condition causal support links of the main model, these links are defined for all types of transitions.

pub mod by_source;
pub mod ground;
pub mod supports;

use crate::analysis::{Source, collect_nonsimple_conditions_and_effects_to_relax};
use crate::{EffectId, EffectOp, IntTerm, SchedEncoder, StateVar, encoder::CondId};

use by_source::BySource;

use aries_solver::core::{Lit, views::Dom};
use idmap::DirectIdMap;

pub type TransitionId = usize;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitionType {
    Cond,
    Eff,
    CondEff,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Transition {
    /// A cond-transition (or simply condition).
    Cond(CondId),
    /// An eff-transition (or simply effect).
    Eff(EffectId),
    /// A condeff-transition (or pure transition), i.e. a pair (condition, effect) where the condition and effect share:
    /// - their source (container action instance),
    /// - presence literal,
    /// - their state variable name (state function),
    /// - and their state variable parameter-variables (arguments).
    CondEff(CondId, EffectId),
}
impl Transition {
    pub fn tpe(&self) -> TransitionType {
        match self {
            Transition::Cond(_) => TransitionType::Cond,
            Transition::Eff(_) => TransitionType::Eff,
            Transition::CondEff(_, _) => TransitionType::CondEff,
        }
    }

    /// Resolves the transition from its condition and/or effect part.
    fn view(self, ctx: &SchedEncoder) -> TransitionView<'_> {
        match self {
            Transition::Cond(cond_id) => {
                let c = ctx.causal_links.conditions.get(cond_id);
                TransitionView {
                    source: c.source,
                    prez: c.prez,
                    state_var: &c.state_var,
                    pre_val: Some(c.value),
                    post_op: EffectOp::Assign(c.value),
                }
            }
            Transition::Eff(eff_id) => {
                let e = ctx.sched.effects.get(eff_id);
                TransitionView {
                    source: e.source,
                    prez: e.prez,
                    state_var: &e.state_var,
                    pre_val: None,
                    post_op: e.operation.clone(),
                }
            }
            Transition::CondEff(cond_id, eff_id) => {
                let c = ctx.causal_links.conditions.get(cond_id);
                let e = ctx.sched.effects.get(eff_id);
                debug_assert!(c.source == e.source && c.prez == e.prez && c.state_var == e.state_var);
                TransitionView {
                    source: e.source,
                    prez: e.prez,
                    state_var: &e.state_var,
                    pre_val: Some(c.value),
                    post_op: e.operation.clone(),
                }
            }
        }
    }
}

struct TransitionView<'a> {
    source: Source,
    prez: Lit,
    state_var: &'a StateVar,
    /// The "pre-value" of the transition (i.e. value the transition's state variable had at the transition's start).
    /// For a [`Transition::Cond`] or [`Transition::CondEff`], corresponds to the value of the condition part (`Some`),
    /// and to `None` for a [`Transition::Eff`].
    pre_val: Option<IntTerm>,
    /// The "post-operation" (or "post-op" / "post-value") of the transition
    /// (i.e. the operation that is applied to (alternatively, the value that is given to) the transition's state variable at the transition's end).
    /// For a [`Transition::Eff`] or [`Transition::CondEff`], corresponds to the operation of the effect part,
    /// and, for a [`Transition::Cond`] to an operation assigning the condition's value.
    post_op: EffectOp,
}
impl<'a> TransitionView<'a> {
    /// The terms of the transition, in this order:
    /// the arguments of its state variable, its pre-value (if any), and the term of its post-operation.
    fn terms(self) -> impl Iterator<Item = IntTerm> + 'a {
        let post_op_term = match self.post_op {
            EffectOp::Assign(term) | EffectOp::Step(term) => term,
        };
        self.state_var
            .args
            .iter()
            .copied()
            .chain(self.pre_val)
            .chain([post_op_term])
    }
}

#[derive(Clone)]
pub(crate) struct Transitions {
    store: Vec<Transition>,

    /// For each transition, the index of each of its terms (see [`TransitionView::terms`]) in its source's terms (`None` for a constant term).
    /// This is needed to facilitate evaluating a transition's terms against a an assignment of its source's terms.
    transition_terms_indices_in_source: Vec<smallvec::SmallVec<[Option<usize>; 4]>>,

    of_condition: DirectIdMap<CondId, TransitionId>,
    of_effect: DirectIdMap<EffectId, TransitionId>,
    of_source: BySource<Vec<TransitionId>>,
}

impl Transitions {
    #[allow(dead_code)]
    pub fn contains(&self, trans_id: TransitionId) -> bool {
        trans_id < self.store.len()
    }
    pub fn get(&self, trans_id: TransitionId) -> Transition {
        self.store[trans_id]
    }
    /// Returns the id of the transition in which a condition appears
    /// (which could be a [`Transition::Cond`] OR a [`Transition::CondEff`] !)
    pub fn of_condition(&self, cond_id: CondId) -> Option<TransitionId> {
        self.of_condition.get(cond_id).copied()
    }
    /// Returns the id of the transition in which an effect appears
    /// (which could be a [`Transition::Effect`] OR a [`Transition::CondEff`] !)
    pub fn of_effect(&self, eff_id: EffectId) -> Option<TransitionId> {
        self.of_effect.get(eff_id).copied()
    }
    /// Returns the ids of transitions defined in a source
    pub fn of_source(&self, source: Source) -> &[TransitionId] {
        self.of_source.get(source).map(|trs| trs.as_slice()).unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = (TransitionId, Transition)> {
        self.store.iter().copied().enumerate()
    }
    #[allow(dead_code)]
    pub fn iter_of_conditions(&self) -> impl Iterator<Item = (CondId, TransitionId)> {
        self.of_condition.iter().map(|(cond_id, &trans_id)| (cond_id, trans_id))
    }
    pub fn iter_of_effects(&self) -> impl Iterator<Item = (EffectId, TransitionId)> {
        self.of_effect.iter().map(|(eff_id, &trans_id)| (eff_id, trans_id))
    }
    pub fn iter_of_sources(&self) -> impl Iterator<Item = (Source, &Vec<TransitionId>)> {
        self.of_source.iter()
    }

    /// Resolves a transition from its condition and / or effect part.
    fn get_view<'a>(&self, trans_id: TransitionId, ctx: &'a SchedEncoder) -> TransitionView<'a> {
        self.store[trans_id].view(ctx)
    }
    pub fn get_prez(&self, trans_id: TransitionId, ctx: &SchedEncoder) -> Lit {
        self.get_view(trans_id, ctx).prez
    }
    pub fn get_source(&self, trans_id: TransitionId, ctx: &SchedEncoder) -> Source {
        self.get_view(trans_id, ctx).source
    }
    pub fn get_state_var<'a>(&self, trans_id: TransitionId, ctx: &'a SchedEncoder) -> &'a StateVar {
        self.get_view(trans_id, ctx).state_var
    }

    /// Whether the transition necessarily ends before the other one starts
    /// (given the bounds of their time points, as per `ctx`).
    pub fn necessarily_precedes(
        &self,
        trans_id: TransitionId,
        other_trans_id: TransitionId,
        ctx: &SchedEncoder,
    ) -> bool {
        let effect_span = |eff_id: EffectId| {
            let eff = ctx.sched.effects.get(eff_id);
            (ctx.lb(eff.transition_start), ctx.ub(eff.mutex_end))
        };
        let condition_span = |cond_id: CondId| {
            let timepoint = ctx.causal_links.conditions.get(cond_id).timepoint;
            (ctx.lb(timepoint), ctx.ub(timepoint))
        };
        let time_span_bounds = |tr_id| match self.store[tr_id] {
            Transition::Eff(eff_id) => effect_span(eff_id),
            Transition::Cond(cond_id) => condition_span(cond_id),
            Transition::CondEff(cond_id, eff_id) => {
                let (cond_start, cond_end) = condition_span(cond_id);
                let (eff_start, eff_end) = effect_span(eff_id);
                (cond_start.min(eff_start), cond_end.max(eff_end))
            }
        };
        let (_, latest_end) = time_span_bounds(trans_id);
        let (earliest_start, _) = time_span_bounds(other_trans_id);
        latest_end < earliest_start
    }

    /// Collects transitions from "unambiguous" (non-"nonsimple") conditions and effects.
    pub fn new_unambiguous(ctx: &SchedEncoder) -> Self {
        // Collects nonsimple transitions to ignore / relax.
        let (conditions_to_ignore, effects_to_ignore) = collect_nonsimple_conditions_and_effects_to_relax(ctx);

        // Group conditions and effects by sources

        let mut conditions: BySource<Vec<_>> = BySource::default();
        for (cond_id, c) in ctx.causal_links.conditions.iter().enumerate() {
            if !conditions_to_ignore.contains(&cond_id) {
                conditions.get_mut_or_default(c.source).push((cond_id, c));
            }
        }
        let mut effects: BySource<Vec<_>> = BySource::default();
        for (eff_id, e) in ctx.sched.effects.iter().enumerate() {
            if !effects_to_ignore.contains(&eff_id) {
                effects.get_mut_or_default(e.source).push((eff_id, e));
            }
        }

        // First, iterate over conditions (grouped by sources) and introduce corresponding Cond transitions.
        // Then, iterate over effects (grouped by sources) and the conditions for those sources.
        // When there is *exactly one* compatible (condition, effect) candidate pair for merging into a CondEff,
        // then a corresponding CondEff transition is introduced, modifying the previously inserted Cond transition.
        // Otherwise, an Eff transition is introduced.

        let mut store = vec![];

        let mut of_condition = DirectIdMap::default();
        let mut of_effect = DirectIdMap::default();
        let mut of_source: BySource<Vec<TransitionId>> = BySource::default();

        for (source, cs) in conditions.iter() {
            for &(cond_id, _) in cs {
                let trans_id = store.len();
                of_condition.insert(cond_id, trans_id);
                of_source.get_mut_or_default(source).push(trans_id);
                store.push(Transition::Cond(cond_id));
            }
        }
        for (source, es) in effects.iter() {
            for &(eff_id, e) in es {
                // A condition and an effect sharing their presence and state variable
                // can be merged into a CondEff ONLY when they are the only candidates !
                // For example, in an ambiguous situation where two conditions could be merged into
                // a CondEff with the same one effect, no merge should be performed.
                let matches_effect = |state_var: &StateVar, prez: Lit| *state_var == e.state_var && prez == e.prez;
                // (No CondEff pattern allowed for the empty source.)
                let paired_cond_id = source.and(conditions.get(source)).and_then(|cs| {
                    let mut conds = cs.iter().filter(|(_, c)| matches_effect(&c.state_var, c.prez));
                    let num_effs = es
                        .iter()
                        .filter(|(_, other)| matches_effect(&other.state_var, other.prez))
                        .count();
                    match (conds.next(), conds.next(), num_effs) {
                        (Some(&(cond_id, _)), None, 1) => Some(cond_id),
                        _ => None,
                    }
                });

                if let Some(cond_id) = paired_cond_id {
                    // Change the previously inserted Cond transition into a CondEff
                    let trans_id = *of_condition.get(cond_id).unwrap();
                    of_effect.insert(eff_id, trans_id);
                    store[trans_id] = Transition::CondEff(cond_id, eff_id);
                } else {
                    // Add a new Eff transition
                    let trans_id = store.len();
                    of_effect.insert(eff_id, trans_id);
                    of_source.get_mut_or_default(source).push(trans_id);
                    store.push(Transition::Eff(eff_id));
                }
            }
        }

        // For each transition, collect its variable terms' indices in the list of its source's variable terms.
        // Note that since we're filtering out "nonsimple" transitions, they MUST appear in them.
        // (this does not apply to constant terms of course, only to variable ones).

        let mut transition_terms_indices_in_source = Vec::with_capacity(store.len());

        let get_source_terms = |source| {
            if let Some(task_id) = source {
                &ctx.sched.tasks[task_id].args
            } else {
                &ctx.sched.global_args
            }
        };

        for transition in store.iter() {
            let view = transition.view(ctx);
            let source_terms = get_source_terms(view.source);

            let index_in_source = |term: IntTerm| -> Option<usize> {
                if term.is_cst() {
                    return None;
                }
                let idx = source_terms.iter().position(|&t| t == term);
                debug_assert!(
                    idx.is_some(),
                    "non-constant transition term absent from its source's args
                    (such transitions are 'nonsimple' and must have been filtered out)"
                );
                idx
            };

            transition_terms_indices_in_source.push(view.terms().map(index_in_source).collect());
        }

        Self {
            store,
            transition_terms_indices_in_source,
            of_condition,
            of_effect,
            of_source,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::collect_nonsimple_conditions_and_effects_to_relax;
    use crate::lprelax::tests::visitall::{VisitAllLine, build_and_encode_visitall_line};
    use crate::lprelax::transitions::{TransitionType, Transitions};

    #[test]
    fn test_transitions_visitall_line() {
        let encoder = build_and_encode_visitall_line(
            &VisitAllLine {
                num_locs: 5,
                num_moves: 4,
            },
            false,
        );

        let transitions = Transitions::new_unambiguous(&encoder);

        assert!({
            let (conditions_to_ignore, effects_to_ignore) = collect_nonsimple_conditions_and_effects_to_relax(&encoder);
            conditions_to_ignore.is_empty() && effects_to_ignore.is_empty()
        });

        assert_eq!(transitions.iter().count(), 31);
        assert_eq!(
            transitions
                .iter()
                .filter(|(_, tr)| tr.tpe() == TransitionType::Cond)
                .count(),
            9
        );
        assert_eq!(
            transitions
                .iter()
                .filter(|(_, tr)| tr.tpe() == TransitionType::Eff)
                .count(),
            18
        );
        assert_eq!(
            transitions
                .iter()
                .filter(|(_, tr)| tr.tpe() == TransitionType::CondEff)
                .count(),
            4
        );
    }
}
