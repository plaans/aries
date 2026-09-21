mod closed_world_default;
pub(crate) mod examples;
pub mod ground;
pub mod supports;

use closed_world_default::ClosedWorldDefaultEffects;

use aries_solver::{core::IntCst, lang::Lit};

use idmap::DirectIdMap;

use crate::analysis::{Source, collect_nonsimple_conditions_and_effects_to_relax};
use crate::{EffectId, EffectOp, IntTerm, SchedEncoder, StateVar, TaskId, constraints::HasValueAt, encoder::CondId};

pub type TransitionId = usize;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitionType {
    Cond,
    Eff,
    CondEff,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Transition {
    Cond(CondId),
    Eff(EffectId),
    /// A condition and effect sharing the same source, presence literal, and state variable.
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
}

/// Invariant: condition transitionss' `op` must be `EffectOp::Assign(val.unwrap())`.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct TransitionTermsView<'a> {
    // tpe: TransitionType,
    args: &'a [IntTerm],
    val: Option<IntTerm>,
    op: EffectOp,
}
impl<'a> TransitionTermsView<'a> {
    // pub fn tpe(&self) -> TransitionType {
    //     self.tpe
    // }
    pub fn args(&'a self) -> &'a [IntTerm] {
        self.args
    }
    pub fn val(&self) -> Option<IntTerm> {
        self.val
    }
    pub fn op(&self) -> &EffectOp {
        &self.op
    }
    #[allow(dead_code)]
    pub fn op_assign(&self) -> IntTerm {
        match &self.op {
            EffectOp::Assign(term) => *term,
            _ => panic!("effect operation must be an assign"),
        }
    }
}

/// An `None` value means the corresponding transition term doesn't actually appear in its source's terms
/// (e.g. because it's a constant, see [`collect_nonsimple_conditions_and_effects_to_relax`]).
#[derive(Clone)]
struct TransitionTermsIndicesInSource {
    pub args: smallvec::SmallVec<[Option<u16>; 4]>,
    pub val: Option<Option<u16>>,
    pub op: Option<u16>,
}

struct EffectBasicInfoView<'a> {
    source: Source,
    prez: Lit,
    state_var: &'a StateVar,
    op: &'a EffectOp,
}
impl crate::Effect {
    fn view<'a>(&'a self) -> EffectBasicInfoView<'a> {
        EffectBasicInfoView {
            source: self.source,
            prez: self.prez,
            state_var: &self.state_var,
            op: &self.operation,
        }
    }
}
#[derive(Clone)]
struct EffectBasicInfo {
    source: Source,
    prez: Lit,
    state_var: StateVar,
    operation: EffectOp,
}
impl EffectBasicInfo {
    fn view<'a>(&'a self) -> EffectBasicInfoView<'a> {
        EffectBasicInfoView {
            source: self.source,
            prez: self.prez,
            state_var: &self.state_var,
            op: &self.operation,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Transitions {
    /// Stores "unambiguous" transitions (i.e. not "nonsimple" ones, see [`collect_nonsimple_conditions_and_effects_to_relax`]).
    store: Vec<Transition>,
    /// For each transition, stores the indices of its terms in its source's terms.
    /// This is needed to evaluate a transition's grounding given a grounding of its source.
    /// (Reminder: one of the requirements of an "unambiguous" transition is that its terms are either constant or appear in its source's terms / arguments).
    transition_terms_indices_in_source: Vec<TransitionTermsIndicesInSource>,

    recovered_closed_world_defaults: ClosedWorldDefaultEffects,

    of_condition: DirectIdMap<CondId, TransitionId>,
    of_effect: DirectIdMap<EffectId, TransitionId>,
    of_concrete_source: DirectIdMap<TaskId, Vec<TransitionId>>,
    of_empty_source: Vec<TransitionId>,
}

impl Transitions {
    #[allow(dead_code)]
    pub fn get(&self, trans_id: TransitionId) -> Transition {
        self.store[trans_id]
    }
    pub fn of_condition(&self, cond_id: CondId) -> Option<TransitionId> {
        self.of_condition.get(cond_id).copied()
    }
    pub fn of_effect(&self, eff_id: EffectId) -> Option<TransitionId> {
        self.of_effect.get(eff_id).copied()
    }
    pub fn of_source(&self, source: Source) -> &[TransitionId] {
        if let Some(task_id) = source {
            self.of_concrete_source
                .get(task_id)
                .map(|trs| trs.as_slice())
                .unwrap_or_default()
        } else {
            &self.of_empty_source
        }
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
        std::iter::chain(
            [(None, &self.of_empty_source)],
            self.of_concrete_source
                .iter()
                .map(|(task_id, trans_ids)| (Some(task_id), trans_ids)),
        )
    }

    pub fn get_condition<'a>(&self, trans_id: TransitionId, ctx: &'a SchedEncoder) -> Option<(CondId, &'a HasValueAt)> {
        match self.store[trans_id] {
            Transition::Cond(cond_id) | Transition::CondEff(cond_id, _) => {
                Some((cond_id, ctx.causal_links.conditions.get(cond_id)))
            }
            Transition::Eff(_) => None,
        }
    }
    fn get_effect_info<'a>(
        &'a self,
        trans_id: TransitionId,
        ctx: &'a SchedEncoder,
    ) -> Option<(EffectId, EffectBasicInfoView<'a>)> {
        match self.store[trans_id] {
            Transition::Eff(eff_id) | Transition::CondEff(_, eff_id) => {
                let info_view: EffectBasicInfoView<'_> = if self.recovered_closed_world_defaults.contains(eff_id) {
                    self.recovered_closed_world_defaults.get(eff_id).view()
                } else {
                    ctx.sched.effects.get(eff_id).view()
                };
                Some((eff_id, info_view))
            }
            Transition::Cond(_) => None,
        }
    }
    pub fn get_prez(&self, trans_id: TransitionId, ctx: &SchedEncoder) -> Lit {
        match self.store[trans_id].tpe() {
            TransitionType::Cond => self.get_condition(trans_id, ctx).unwrap().1.prez,
            TransitionType::Eff => self.get_effect_info(trans_id, ctx).unwrap().1.prez,
            TransitionType::CondEff => {
                let res = self.get_effect_info(trans_id, ctx).unwrap().1.prez;
                debug_assert!(res == self.get_condition(trans_id, ctx).unwrap().1.prez);
                res
            }
        }
    }
    pub fn get_source(&self, trans_id: TransitionId, ctx: &SchedEncoder) -> Source {
        match self.store[trans_id].tpe() {
            TransitionType::Cond => self.get_condition(trans_id, ctx).unwrap().1.source,
            TransitionType::Eff => self.get_effect_info(trans_id, ctx).unwrap().1.source,
            TransitionType::CondEff => {
                let res = self.get_effect_info(trans_id, ctx).unwrap().1.source;
                debug_assert!(res == self.get_condition(trans_id, ctx).unwrap().1.source);
                res
            }
        }
    }
    pub fn get_state_var<'a>(&'a self, trans_id: TransitionId, ctx: &'a SchedEncoder) -> &'a StateVar {
        match self.store[trans_id].tpe() {
            TransitionType::Cond => &self.get_condition(trans_id, ctx).unwrap().1.state_var,
            TransitionType::Eff => self.get_effect_info(trans_id, ctx).unwrap().1.state_var,
            TransitionType::CondEff => {
                let res = self.get_effect_info(trans_id, ctx).unwrap().1.state_var;
                debug_assert!(*res == self.get_condition(trans_id, ctx).unwrap().1.state_var);
                res
            }
        }
    }

    pub(self) fn get_terms<'a>(&'a self, trans_id: TransitionId, ctx: &'a SchedEncoder) -> TransitionTermsView<'a> {
        let args = self.get_state_var(trans_id, ctx).args.as_slice();
        match self.store[trans_id].tpe() {
            TransitionType::Cond => {
                let val = self.get_condition(trans_id, ctx).unwrap().1.value;
                TransitionTermsView {
                    args,
                    val: Some(val),
                    op: EffectOp::Assign(val),
                }
            }
            TransitionType::Eff => {
                let op = self.get_effect_info(trans_id, ctx).unwrap().1.op.clone();
                TransitionTermsView { args, val: None, op }
            }
            TransitionType::CondEff => {
                let val = self.get_condition(trans_id, ctx).unwrap().1.value;
                let op = self.get_effect_info(trans_id, ctx).unwrap().1.op.clone();
                TransitionTermsView {
                    args,
                    val: Some(val),
                    op,
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_recovered_closed_world_default(&self, eff_id: EffectId) -> bool {
        self.recovered_closed_world_defaults.contains(eff_id)
    }
    pub fn are_recovered_closed_world_default_effects_empty(&self) -> bool {
        self.recovered_closed_world_defaults.is_empty()
    }

    /// Collects transitions from "unambiguous" conditions and effects (i.e. filtering out "nonsimple" ones)
    ///
    /// The context borrow is mutable to create new 'mutex end' variables for the recovered missing initial effects.
    pub fn new_unambiguous(ctx: &SchedEncoder, recover_closed_world_defaults: bool) -> Self {
        // Collects nonsimple transitions to ignore / relax.
        let (conditions_to_ignore, effects_to_ignore) = collect_nonsimple_conditions_and_effects_to_relax(ctx);

        // Group conditions and effects by sources

        let mut empty_source_conditions = vec![];
        let mut concrete_source_conditions = DirectIdMap::default();
        let mut empty_source_effects = vec![];
        let mut concrete_source_effects = DirectIdMap::default();

        for (cond_id, c) in ctx.causal_links.conditions.iter().enumerate() {
            if conditions_to_ignore.contains(&cond_id) {
                continue;
            }
            if let Some(task_id) = c.source {
                if !concrete_source_conditions.contains_key(task_id) {
                    concrete_source_conditions.insert(task_id, vec![]);
                }
                concrete_source_conditions.get_mut(task_id).unwrap().push((cond_id, c));
            } else {
                empty_source_conditions.push((cond_id, c));
            }
        }
        for (eff_id, e) in ctx.sched.effects.iter().enumerate() {
            if effects_to_ignore.contains(&eff_id) {
                continue;
            }
            if let Some(task_id) = e.source {
                if !concrete_source_effects.contains_key(task_id) {
                    concrete_source_effects.insert(task_id, vec![]);
                }
                concrete_source_effects.get_mut(task_id).unwrap().push((eff_id, e));
            } else {
                empty_source_effects.push((eff_id, e));
            }
        }

        // First, iterate over conditions (grouped by sources) and introduce corresponding Cond transitions.
        // Then, iterate over effects (grouped by sources) and the conditions for those sources.
        // When a compatible condition and effect are found, a corresponding CondEff transition is introduced,
        // modifying the previously inserted Cond transition.
        // If no compatible condition is found, a Eff transition is introduced.
        //
        // If a ground initial (empty source) Eff transition is introduced, remember that grounding.
        // This is needed to avoid overriding it later when recovering "missing" closed world default initial effects.

        let mut store = vec![];

        let mut of_condition = DirectIdMap::default();
        let mut of_effect = DirectIdMap::default();
        let mut of_empty_source = vec![];
        let mut of_concrete_source = DirectIdMap::default();

        let mut initial_effects_ground_args =
            std::collections::HashMap::<crate::Sym, Vec<smallvec::SmallVec<[IntCst; 4]>>>::new();

        let source_conds_iter = std::iter::chain(
            [(None, &empty_source_conditions)],
            concrete_source_conditions
                .iter()
                .map(|(task_id, conds)| (Some(task_id), conds)),
        );
        let source_effs_iter = std::iter::chain(
            [(None, &empty_source_effects)],
            concrete_source_effects
                .iter()
                .map(|(task_id, effs)| (Some(task_id), effs)),
        );

        for (source, cs) in source_conds_iter {
            if let Some(task_id) = source
                && !of_concrete_source.contains_key(task_id)
            {
                of_concrete_source.insert(task_id, vec![]);
            }
            for &(cond_id, _) in cs {
                let trans_id = store.len();
                of_condition.insert(cond_id, trans_id);
                if let Some(task_id) = source {
                    of_concrete_source.get_mut(task_id).unwrap().push(trans_id);
                } else {
                    of_empty_source.push(trans_id);
                }
                store.push(Transition::Cond(cond_id));
            }
        }
        for (source, es) in source_effs_iter {
            if let Some(task_id) = source
                && !of_concrete_source.contains_key(task_id)
            {
                of_concrete_source.insert(task_id, vec![]);
            }
            for &(eff_id, e) in es {
                let mut compatible_conds_found = 0;

                // No CondEff pattern allowed for empty source.
                if source.is_some() {
                    let cs = if let Some(task_id) = source {
                        concrete_source_conditions.get(task_id)
                    } else {
                        Some(&empty_source_conditions)
                    }
                    .into_iter()
                    .flatten();

                    for &(cond_id, c) in cs {
                        if e.state_var == c.state_var && e.prez == c.prez {
                            // Change the previously inserted Cond transition into a CondEff
                            let trans_id = *of_condition.get(cond_id).unwrap();
                            of_effect.insert(eff_id, trans_id);
                            store[trans_id] = Transition::CondEff(cond_id, eff_id);

                            compatible_conds_found += 1;
                        }
                    }
                    debug_assert!(compatible_conds_found <= 1);
                }

                // Add a new Eff transition if the effect doesn't correspond to a CondEff
                if compatible_conds_found == 0 {
                    let trans_id = store.len();
                    of_effect.insert(eff_id, trans_id);
                    if let Some(task_id) = source {
                        of_concrete_source.get_mut(task_id).unwrap().push(trans_id);
                    } else {
                        of_empty_source.push(trans_id);
                    }
                    store.push(Transition::Eff(eff_id));
                }

                // Remember the args groundings of ground initial effects
                if recover_closed_world_defaults
                    && source.is_none()
                    && e.state_var.args.iter().all(|term| term.is_cst())
                {
                    let ground_args = e.state_var.args.iter().map(|term| term.constant).collect();
                    initial_effects_ground_args
                        .entry(e.state_var.fluent.to_string())
                        .or_default()
                        .push(ground_args);
                    debug_assert!({
                        use itertools::Itertools;
                        initial_effects_ground_args
                            .get(&e.state_var.fluent)
                            .unwrap()
                            .iter()
                            .all_unique()
                    });
                }
            }
        }

        // Loop over fluents and their parameter types' ground values.
        // For each such grounding, introduce a default-valued initial effect (closed world default),
        // if there wasn't already an effect with the same ground parameters encountered earlier
        // (among the "explicit" known initial effects accessible from `ctx`).

        let recovered_closed_world_defaults = if !recover_closed_world_defaults {
            ClosedWorldDefaultEffects::default()
        } else {
            let mut recovered_closed_world_defaults = ClosedWorldDefaultEffects::new(
                ctx,
                effects_to_ignore,
                conditions_to_ignore,
                initial_effects_ground_args,
            );

            for (sym, params, _) in ctx.sched.fluents.iter() {
                if recovered_closed_world_defaults.ignored_fluents.contains(sym) {
                    continue;
                }

                let args = crate::boxes::BBox::new(params.iter().map(|p| p.range).collect::<Vec<_>>());
                let mut grs = args.as_ref().points();
                while let Some(gr) = streaming_iterator::StreamingIterator::next(&mut grs) {
                    let args_ground = Vec::from_iter(gr.iter().copied());

                    if let Ok(eff_id) = recovered_closed_world_defaults.add(
                        sym.to_string(),
                        args_ground,
                        ctx.sched.fluents.get_return(sym).unwrap().range.first,
                    ) {
                        let tr_id = store.len();
                        of_effect.insert(eff_id, tr_id);
                        of_empty_source.push(tr_id);
                        store.push(Transition::Eff(eff_id));
                    } else {
                        // Ignored (not added) as there already in an initial effect with these ground args.
                    };
                }
            }

            recovered_closed_world_defaults
        };

        // For each transition, collect its terms' (args and values) indices in the list of its source's args.
        //
        // Note that currently, transitions whose terms contain auxiliary or reification variables
        // that do not appearing in the the source's args are ignored anyway (filtered out as "nonsimple")

        let mut transition_terms_indices_in_source = Vec::with_capacity(store.len());

        let get_source_terms = |source| {
            if let Some(task_id) = source {
                &ctx.sched.tasks[task_id].args
            } else {
                &ctx.sched.global_args
            }
        };
        let get_effect_info = |eff_id| {
            if recovered_closed_world_defaults.contains(eff_id) {
                recovered_closed_world_defaults.get(eff_id).view()
            } else {
                ctx.sched.effects.get(eff_id).view()
            }
        };
        let get_source = |transition| match transition {
            Transition::Cond(cond_id) => ctx.causal_links.conditions.get(cond_id).source,
            Transition::Eff(eff_id) => get_effect_info(eff_id).source,
            Transition::CondEff(cond_id, eff_id) => {
                let res = ctx.causal_links.conditions.get(cond_id).source;
                debug_assert!(res == get_effect_info(eff_id).source);
                debug_assert!(!recovered_closed_world_defaults.contains(eff_id));
                res
            }
        };
        let get_transition_terms = |transition| match transition {
            Transition::Cond(cond_id) => TransitionTermsView {
                args: &ctx.causal_links.conditions.get(cond_id).state_var.args,
                val: Some(ctx.causal_links.conditions.get(cond_id).value),
                op: EffectOp::Assign(ctx.causal_links.conditions.get(cond_id).value),
            },
            Transition::Eff(eff_id) => TransitionTermsView {
                args: &get_effect_info(eff_id).state_var.args,
                val: None,
                op: get_effect_info(eff_id).op.clone(),
            },
            Transition::CondEff(cond_id, eff_id) => {
                debug_assert!(!recovered_closed_world_defaults.contains(eff_id));
                TransitionTermsView {
                    args: &ctx.causal_links.conditions.get(cond_id).state_var.args,
                    val: Some(ctx.causal_links.conditions.get(cond_id).value),
                    op: get_effect_info(eff_id).op.clone(),
                }
            }
        };

        for transition in store.iter() {
            let source_terms = get_source_terms(get_source(*transition));
            let transition_terms = get_transition_terms(*transition);

            let index_in_source = |term: IntTerm| -> Option<u16> {
                if term.is_cst() {
                    return None;
                }
                let idx = source_terms.iter().position(|&t| t == term);
                debug_assert!(
                    idx.is_some(),
                    "non-constant transition term absent from its source's args (such transitions are 'nonsimple' and must have been filtered out)"
                );
                idx.map(|idx| idx as u16)
            };

            let transition_args_indices_in_source =
                transition_terms.args().iter().copied().map(index_in_source).collect();
            let transition_val_index_in_source = transition_terms.val().map(index_in_source);
            let transition_op_index_in_source = match transition_terms.op() {
                EffectOp::Assign(term) => index_in_source(*term),
                EffectOp::Step(_) => todo!(),
            };

            transition_terms_indices_in_source.push(TransitionTermsIndicesInSource {
                args: transition_args_indices_in_source,
                val: transition_val_index_in_source,
                op: transition_op_index_in_source,
            });
        }

        Self {
            store,
            transition_terms_indices_in_source,
            of_condition,
            of_effect,
            of_empty_source,
            of_concrete_source,
            recovered_closed_world_defaults,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{
        collect_nonsimple_conditions_and_effects_to_relax,
        transitions::{
            TransitionType, Transitions,
            examples::visitall::{VisitAllLine, build_and_encode_visitall_line},
        },
    };

    #[test]
    fn test_transitions_visitall_line() {
        let encoder = build_and_encode_visitall_line(
            &VisitAllLine {
                num_locs: 5,
                num_moves: 4,
            },
            false,
        );

        let transitions = Transitions::new_unambiguous(&encoder, true);

        assert!({
            let (conditions_to_ignore, effects_to_ignore) = collect_nonsimple_conditions_and_effects_to_relax(&encoder);
            conditions_to_ignore.is_empty() && effects_to_ignore.is_empty()
        });

        assert_eq!(transitions.iter().count(), 56);
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
            43
        );
        assert_eq!(
            transitions
                .iter()
                .filter(|(_, tr)| tr.tpe() == TransitionType::CondEff)
                .count(),
            4
        );

        assert_eq!(
            transitions
                .iter_of_effects()
                .filter(|&(eff_id, _)| transitions.is_recovered_closed_world_default(eff_id))
                .count(),
            25
        );
    }
}
