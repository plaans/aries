mod ground;
mod missing_initial_effects;

pub use ground::*;
use missing_initial_effects::RecoveredMissingInitialEffects;

use aries_solver::lang::Lit;

use idmap::DirectIdMap;

use crate::ext::{Source, SourceGrounding, collect_nonsimple_conditions_and_effects_to_relax};
use crate::{
    Effect, EffectId, IntTerm, StateVar, TaskId,
    constraints::HasValueAt,
    encoder::{CondId, SchedEncoder},
};

pub type TransitionId = usize;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Transition {
    Cond(CondId),
    Eff(EffectId),
    /// A condition and effect sharing the same source, presence literal, and state variable.
    CondEff(CondId, EffectId),
}

pub(crate) struct TransitionTermsView<'a> {
    pub args: &'a [IntTerm],
    pub valfrom: Option<IntTerm>,
    pub valto: Option<IntTerm>,
}

#[derive(Clone)]
struct TransitionTermsIndicesInSource {
    pub args: smallvec::SmallVec<[Option<usize>; 4]>,
    pub valfrom: Option<Option<usize>>,
    pub valto: Option<Option<usize>>,
}

#[derive(Clone)]
pub(crate) struct Transitions {
    store: Vec<Transition>,
    transition_terms_indices_in_source: Vec<TransitionTermsIndicesInSource>,

    of_condition: DirectIdMap<CondId, TransitionId>,
    of_effect: DirectIdMap<EffectId, TransitionId>,
    of_concrete_source: DirectIdMap<TaskId, Vec<TransitionId>>,
    of_empty_source: Vec<TransitionId>,

    recovered_mies: Option<RecoveredMissingInitialEffects>,
}

impl std::ops::Index<TransitionId> for Transitions {
    type Output = Transition;

    fn index(&self, index: TransitionId) -> &Self::Output {
        &self.store[index]
    }
}

impl Transitions {
    pub fn is_pure_cond(&self, transition_id: TransitionId) -> bool {
        matches!(self.store[transition_id], Transition::Cond(_))
    }
    pub fn is_pure_eff(&self, transition_id: TransitionId) -> bool {
        matches!(self.store[transition_id], Transition::Eff(_))
    }
    pub fn is_condeff(&self, transition_id: TransitionId) -> bool {
        matches!(self.store[transition_id], Transition::CondEff(_, _))
    }

    pub fn get(&self, transition_id: TransitionId) -> Transition {
        self.store[transition_id]
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

    pub fn iter(&self) -> impl Iterator<Item = (TransitionId, Transition)> {
        self.store.iter().copied().enumerate()
    }
    // pub fn iter_of_conditions(&self) -> impl Iterator<Item = (CondId, TransitionId)> {
    //     self.of_condition
    //         .iter()
    //         .map(|(cond_id, &transition_id)| (cond_id, transition_id))
    // }
    pub fn iter_of_effects(&self) -> impl Iterator<Item = (EffectId, TransitionId)> {
        self.of_effect
            .iter()
            .map(|(eff_id, &transition_id)| (eff_id, transition_id))
    }
    // pub fn iter_of_sources(&self) -> impl Iterator<Item = (Source, &Vec<TransitionId>)> {
    //     std::iter::chain(
    //         [(None, &self.of_empty_source)],
    //         self.of_concrete_source
    //             .iter()
    //             .map(|(task_id, entries)| (Some(task_id), entries)),
    //     )
    // }
    /*pub fn iter_sources(&self) -> impl Iterator<Item = Source> {
        std::iter::chain(
            [None],
            self.of_concrete_source.iter().map(|(task_id, _)| Some(task_id))
        )
    }
    pub fn iter_transitions_of_sources(&self) -> impl Iterator<Item = (Source, &Vec<TransitionId>)> {
        self.iter_sources()
            .map(|source| {
                let res = if let Some(task_id) = source {
                    &self.of_concrete_source[task_id]
                } else {
                    &self.of_empty_source
                };
                (source, res)
            })
    }*/

    /*pub fn get_state_var_args_with_source_indices<'a>(
        &'a self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoder,
    ) -> (&'a [IntTerm], &'a [Option<usize>]) {
        (
            &self.get_state_var(transition_id, ctx).args,
            &self.transition_terms_indices_in_source[transition_id].0,
        )
    }
    pub fn get_valfrom_with_source_indices<'a>(
        &'a self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoder,
    ) -> Option<(IntTerm, Option<usize>)> {
        self.transition_terms_indices_in_source[transition_id].1.map(|i| (self.get_valfrom(transition_id, ctx).unwrap(), i))
    }
    pub fn get_valto_with_source_indices<'a>(
        &'a self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoder,
    ) -> Option<(IntTerm, Option<usize>)> {
        self.transition_terms_indices_in_source[transition_id].2.map(|i| (self.get_valto(transition_id, ctx).unwrap(), i))
    }*/
    pub fn get_condition<'a>(
        &self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoder,
    ) -> Option<(CondId, &'a HasValueAt)> {
        match self.get(transition_id) {
            Transition::Cond(cond_id) | Transition::CondEff(cond_id, _) => {
                Some((cond_id, ctx.causal_links.conditions.get(cond_id)))
            }
            Transition::Eff(_) => None,
        }
    }
    pub fn get_effect<'a>(
        &'a self,
        transition_id: TransitionId,
        ctx: &'a SchedEncoder,
    ) -> Option<(EffectId, &'a Effect)> {
        match self.get(transition_id) {
            Transition::Eff(eff_id) | Transition::CondEff(_, eff_id) => {
                let eff = if let Some(recovered_mies) = &self.recovered_mies
                    && recovered_mies.contains(eff_id)
                {
                    recovered_mies.get(eff_id)
                } else {
                    ctx.sched.effects.get(eff_id)
                };
                Some((eff_id, eff))
            }
            Transition::Cond(_) => None,
        }
    }
    // pub fn is_effect_non_missing_initial(&self, eff_id: EffectId) -> bool {
    //     self.recovered_mies
    //         .as_ref()
    //         .is_some_and(|recovered_mies| recovered_mies.contains(eff_id))
    // }
    pub fn get_prez(&self, transition_id: TransitionId, ctx: &SchedEncoder) -> Lit {
        match self.get(transition_id) {
            Transition::Cond(_) => self.get_condition(transition_id, ctx).unwrap().1.prez,
            Transition::Eff(_) => self.get_effect(transition_id, ctx).unwrap().1.prez,
            Transition::CondEff(_, _) => {
                let res = self.get_effect(transition_id, ctx).unwrap().1.prez;
                debug_assert!(res == self.get_condition(transition_id, ctx).unwrap().1.prez);
                res
            }
        }
    }
    pub fn get_source(&self, transition_id: TransitionId, ctx: &SchedEncoder) -> Source {
        match self.get(transition_id) {
            Transition::Cond(_) => self.get_condition(transition_id, ctx).unwrap().1.source,
            Transition::Eff(_) => self.get_effect(transition_id, ctx).unwrap().1.source,
            Transition::CondEff(_, _) => {
                let res = self.get_effect(transition_id, ctx).unwrap().1.source;
                debug_assert!(res == self.get_condition(transition_id, ctx).unwrap().1.source);
                res
            }
        }
    }
    pub fn get_state_var<'a>(&'a self, transition_id: TransitionId, ctx: &'a SchedEncoder) -> &'a StateVar {
        match self.get(transition_id) {
            Transition::Cond(_) => &self.get_condition(transition_id, ctx).unwrap().1.state_var,
            Transition::Eff(_) => &self.get_effect(transition_id, ctx).unwrap().1.state_var,
            Transition::CondEff(_, _) => {
                let res = &self.get_effect(transition_id, ctx).unwrap().1.state_var;
                debug_assert!(*res == self.get_condition(transition_id, ctx).unwrap().1.state_var);
                res
            }
        }
    }
    pub fn get_valfrom(&self, transition_id: TransitionId, ctx: &SchedEncoder) -> Option<IntTerm> {
        match self.get(transition_id) {
            Transition::Cond(_) | Transition::CondEff(_, _) => {
                Some(self.get_condition(transition_id, ctx).unwrap().1.value)
            }
            Transition::Eff(_) => None,
        }
    }
    pub fn get_valto(&self, transition_id: TransitionId, ctx: &SchedEncoder) -> Option<IntTerm> {
        match self.get(transition_id) {
            Transition::Eff(_) | Transition::CondEff(_, _) => {
                match self.get_effect(transition_id, ctx).unwrap().1.operation {
                    crate::EffectOp::Assign(term) => Some(term),
                    crate::EffectOp::Step(_) => todo!(),
                }
            }
            Transition::Cond(_) => None,
        }
    }

    pub fn get_terms<'a>(&'a self, transition_id: TransitionId, ctx: &'a SchedEncoder) -> TransitionTermsView<'a> {
        TransitionTermsView {
            args: &self.get_state_var(transition_id, ctx).args,
            valfrom: self.get_valfrom(transition_id, ctx),
            valto: self.get_valto(transition_id, ctx),
        }
    }

    pub fn get_terms_eval_in_ground_source<'a>(
        &'a self,
        transition_id: TransitionId,
        source_grounding: &SourceGrounding,
        ctx: &'a SchedEncoder,
    ) -> TransitionGrounding {
        let terms = self.get_terms(transition_id, ctx);

        let args = self.transition_terms_indices_in_source[transition_id]
            .args
            .iter()
            .enumerate()
            .map(|(j, i)| i.map_or(terms.args[j].constant, |i| source_grounding[i]))
            .collect();

        let valfrom = self.transition_terms_indices_in_source[transition_id]
            .valfrom
            .map(|i| i.map_or(terms.valfrom.unwrap().constant, |i| source_grounding[i]));

        let valto = self.transition_terms_indices_in_source[transition_id]
            .valto
            .map(|i| i.map_or(terms.valto.unwrap().constant, |i| source_grounding[i]));

        TransitionGrounding { args, valfrom, valto }
    }

    /// Whether (all) "missing" initial effects are used / included
    pub fn includes_recovered_mies(&self) -> bool {
        self.recovered_mies.is_some()
    }

    /// Collects transitions from "unambiguous" conditions and effects (i.e. filtering out "nonsimple" ones)
    ///
    /// The context borrow is mutable to create new 'mutex end' variables for the recovered missing initial effects.
    pub fn new_unambiguous(ctx: &mut SchedEncoder, recover_missing_initial_effects: bool) -> Self {
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
        // This is needed to avoid overriding it later when introducing the default (negative) ground initial effects.

        let mut store = vec![];

        let mut of_condition = DirectIdMap::default();
        let mut of_effect = DirectIdMap::default();
        let mut of_empty_source = vec![];
        let mut of_concrete_source = DirectIdMap::default();

        let mut initial_effects_ground_args =
            std::collections::HashMap::<crate::Sym, Vec<Vec<aries_solver::core::IntCst>>>::new();

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

        for (src, cs) in source_conds_iter {
            for &(cond_id, _) in cs {
                let transition_id = store.len();
                of_condition.insert(cond_id, transition_id);
                if let Some(task_id) = src {
                    if !of_concrete_source.contains_key(task_id) {
                        of_concrete_source.insert(task_id, vec![]);
                    }
                    of_concrete_source.get_mut(task_id).unwrap().push(transition_id);
                } else {
                    of_empty_source.push(transition_id);
                }
                store.push(Transition::Cond(cond_id));
            }
        }
        for (source, es) in source_effs_iter {
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
                            let transition_id = *of_condition.get(cond_id).unwrap();
                            of_effect.insert(eff_id, transition_id);
                            store[transition_id] = Transition::CondEff(cond_id, eff_id);

                            compatible_conds_found += 1;
                        }
                    }
                    debug_assert!(compatible_conds_found <= 1);
                }

                // Add a new Eff transition if the effect doesn't correspond to a CondEff
                if compatible_conds_found == 0 {
                    let transition_id = store.len();
                    of_effect.insert(eff_id, transition_id);
                    if let Some(task_id) = source {
                        if !of_concrete_source.contains_key(task_id) {
                            of_concrete_source.insert(task_id, vec![]);
                        }
                        of_concrete_source.get_mut(task_id).unwrap().push(transition_id);
                    } else {
                        of_empty_source.push(transition_id);
                    }
                    store.push(Transition::Eff(eff_id));
                }

                // Remember the args groundings of ground initial effects
                if recover_missing_initial_effects
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
        // For each such grounding, introduce an initial effect (with default value),
        // if there wasn't already an effect with the same ground parameters encountered earlier
        // (among the "explicit" known initial effects accessible from `ctx`).

        let recovered_mies = if !recover_missing_initial_effects {
            RecoveredMissingInitialEffects::default()
        } else {
            let mut recovered_mies = RecoveredMissingInitialEffects::new(
                ctx,
                effects_to_ignore,
                conditions_to_ignore,
                initial_effects_ground_args,
            );

            for (sym, params, _) in ctx.sched.fluents.iter() {
                if recovered_mies.ignored_fluents.contains(sym) {
                    continue;
                }

                let args = crate::boxes::BBox::new(params.iter().map(|p| p.range).collect::<Vec<_>>());
                let mut grs = args.as_ref().points();
                while let Some(gr) = streaming_iterator::StreamingIterator::next(&mut grs) {
                    let args_ground = Vec::from_iter(gr.iter().copied());

                    if let Ok(eff_id) = recovered_mies.add(
                        sym.to_string(),
                        args_ground,
                        ctx.sched.fluents.get_return(sym).unwrap().range.first,
                        &mut ctx.store,
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

            recovered_mies
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
        let get_effect = |eff_id| {
            if recovered_mies.contains(eff_id) {
                recovered_mies.get(eff_id)
            } else {
                ctx.sched.effects.get(eff_id)
            }
        };
        let get_source = |transition| match transition {
            Transition::Cond(cond_id) => ctx.causal_links.conditions.get(cond_id).source,
            Transition::Eff(eff_id) => get_effect(eff_id).source,
            Transition::CondEff(cond_id, eff_id) => {
                let res = ctx.causal_links.conditions.get(cond_id).source;
                debug_assert!(res == get_effect(eff_id).source);
                debug_assert!(!recovered_mies.contains(eff_id));
                res
            }
        };
        let get_transition_terms = |transition| match transition {
            Transition::Cond(cond_id) => (
                &ctx.causal_links.conditions.get(cond_id).state_var.args,
                Some(ctx.causal_links.conditions.get(cond_id).value),
                None,
            ),
            Transition::Eff(eff_id) => (
                &get_effect(eff_id).state_var.args,
                None,
                Some(match get_effect(eff_id).operation {
                    crate::EffectOp::Assign(term) => term,
                    crate::EffectOp::Step(_) => todo!(),
                }),
            ),
            Transition::CondEff(cond_id, eff_id) => {
                debug_assert!(!recovered_mies.contains(eff_id));
                (
                    &get_effect(eff_id).state_var.args,
                    Some(ctx.causal_links.conditions.get(cond_id).value),
                    Some(match get_effect(eff_id).operation {
                        crate::EffectOp::Assign(term) => term,
                        crate::EffectOp::Step(_) => todo!(),
                    }),
                )
            }
        };

        for transition in store.iter() {
            let source_terms = get_source_terms(get_source(*transition));
            let (transition_args, transition_valfrom, transition_valto) = get_transition_terms(*transition);

            let index_in_source = |term: IntTerm| -> Option<usize> {
                if term.is_cst() {
                    return None;
                }
                let idx = source_terms.iter().position(|&t| t == term);
                debug_assert!(
                    idx.is_some(),
                    "non-constant transition term absent from its source's args (such transitions are 'nonsimple' and must have been filtered out)"
                );
                idx
            };

            let transition_args_indices_in_source = transition_args.iter().copied().map(index_in_source).collect();
            let transition_valfrom_index_in_source = transition_valfrom.map(index_in_source);
            let transition_valto_index_in_source = transition_valto.map(index_in_source);

            transition_terms_indices_in_source.push(TransitionTermsIndicesInSource {
                args: transition_args_indices_in_source,
                valfrom: transition_valfrom_index_in_source,
                valto: transition_valto_index_in_source,
            });
        }

        Self {
            store,
            transition_terms_indices_in_source,
            of_condition,
            of_effect,
            of_empty_source,
            of_concrete_source,
            recovered_mies: recover_missing_initial_effects.then_some(recovered_mies),
        }
    }
}
