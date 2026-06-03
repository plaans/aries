use std::collections::{HashMap, HashSet};

use aries::prelude::*;
use aries::{core::views::Dom, model::lang::ModelWrapper, utils::StreamingIterator};
use idmap::{DirectIdMap, intid::IntegerId};
use itertools::Itertools;
use smallvec::SmallVec;

use crate::constraints::HasValueAt;
use crate::encoder::{CondId, SchedEncoder};
use crate::ext::ground::{SourceGrounding, SourceGroundingFlatId};
use crate::ext::lprelax::ground::{TransitionGrounding, TransitionGroundingFlatId};
use crate::ext::lprelax::transition::*;
use crate::ext::{Source, collect_ambiguous_conditions_and_effects_to_relax};
use crate::{Effect, EffectId, IntTerm, StateVar, Sym, Task, TaskId};

pub(crate) struct LpRelaxSchedEncoder<'a> {
    pub main: &'a mut SchedEncoder,

    transitions: Transitions,

    // sources_grounder: todo!();
    transitions_terms_ranges: Vec<SmallVec<[(IntCst, IntCst); 4]>>,
    empty_source_terms_ranges: Vec<(IntCst, IntCst)>,
    concrete_sources_terms_ranges: Vec<SmallVec<[(IntCst, IntCst); 4]>>,

    pub lprelax: Option<aries_lprelax::LpRelax>,
}

impl<'a> Dom for LpRelaxSchedEncoder<'a> {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.main.upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.main.presence(var)
    }
}
impl<'a> ModelWrapper for LpRelaxSchedEncoder<'a> {
    type Lbl = String;

    fn get_model(&self) -> &crate::Model {
        self.main.get_model()
    }

    fn get_model_mut(&mut self) -> &mut crate::Model {
        self.main.get_model_mut()
    }
}

impl<'a> LpRelaxSchedEncoder<'a> {
    pub fn new(sched_encoder: &'a mut SchedEncoder) -> Self {
        let transitions = Transitions::new_unambiguous(sched_encoder);

        let get_transition_ref = |tr: Transition| match tr {
            Transition::Cond(c_id) => TransitionRef::Cond(sched_encoder.causal_links.conditions.get(c_id)),
            Transition::Eff(e_id) => {
                let e = if e_id < transitions.first_default_initial_effect_id {
                    sched_encoder.sched.effects.get(e_id)
                } else {
                    &transitions.default_initial_effects[e_id - transitions.first_default_initial_effect_id]
                };
                TransitionRef::Eff(e)
            }
            Transition::CondEff(c_id, e_id) => {
                let c = sched_encoder.causal_links.conditions.get(c_id);
                let e = if e_id < transitions.first_default_initial_effect_id {
                    sched_encoder.sched.effects.get(e_id)
                } else {
                    &transitions.default_initial_effects[e_id - transitions.first_default_initial_effect_id]
                };
                TransitionRef::CondEff(c, e)
            }
        };

        let transitions_terms_ranges = transitions
            .store
            .iter()
            .map(|&tr| {
                let tr_ref = get_transition_ref(tr);
                // tr_ref.iter_terms().map(|t| sched_encoder.bounds(t)).collect()
                let n = if tr_ref.get_valfrom().is_some() && tr_ref.get_valto().is_some() {
                    2
                } else {
                    1
                };
                sched_encoder
                    .sched
                    .fluents
                    .get_params(&tr_ref.get_state_var().fluent)
                    .unwrap()
                    .iter()
                    .chain(vec![
                        sched_encoder
                            .sched
                            .fluents
                            .get_return(&tr_ref.get_state_var().fluent)
                            .unwrap();
                        n
                    ])
                    .map(|param| (param.range.first, param.range.last))
                    .collect()
            })
            .collect();

        let empty_source_terms_ranges = sched_encoder
            .sched
            .global_args
            .iter()
            .map(|t| sched_encoder.bounds(t))
            .collect();

        let concrete_sources_terms_ranges = sched_encoder
            .sched
            .tasks
            .iter()
            .map(|task| task.args.iter().map(|t| sched_encoder.bounds(t)).collect())
            .collect();

        Self {
            main: sched_encoder,
            transitions,
            transitions_terms_ranges,
            empty_source_terms_ranges,
            concrete_sources_terms_ranges,
            lprelax: None,
        }
    }

    pub fn iter_sources(&self) -> impl Iterator<Item = Source> {
        std::iter::chain(
            [None],
            self.main
                .sched
                .tasks
                .iter()
                .enumerate()
                .map(|(task_id, _)| Some(TaskId::from_int(u32::try_from(task_id).unwrap()))),
        )
    }
    pub fn iter_nondefault_effects(&self) -> impl Iterator<Item = (EffectId, &Effect)> {
        // self.main.sched.effects.iter().enumerate()
        self.transitions
            .of_effect
            .iter()
            .take(self.transitions.first_default_initial_effect_id)
            .map(|(eff_id, _)| (eff_id, self.main.sched.effects.get(eff_id)))
    }
    pub fn iter_default_effects(&self) -> impl Iterator<Item = (EffectId, &Effect)> {
        self.transitions
            .default_initial_effects
            .iter()
            .enumerate()
            .inspect(|(_, e)| debug_assert!(matches!(e.operation, crate::EffectOp::Assign(_))))
            .map(|(i, e)| (i + self.transitions.first_default_initial_effect_id, e))
    }
    // pub fn iter_effects(&self) -> impl Iterator<Item = (EffectId, &Effect)> {
    //     self.iter_nondefault_effects().chain(self.iter_default_effects())
    // }
    pub fn iter_conditions(&self) -> impl Iterator<Item = (CondId, &HasValueAt)> {
        // self.main.causal_links.conditions.iter().enumerate()
        self.transitions
            .of_condition
            .iter()
            .map(|(cond_id, _)| (cond_id, self.main.causal_links.conditions.get(cond_id)))
    }
    pub fn iter_transitions(&self) -> impl Iterator<Item = (TransitionId, (Transition, Source))> {
        std::iter::chain(
            self.transitions
                .of_empty_source
                .iter()
                .map(|&tr_id| (tr_id, (self.transitions.store[tr_id], None))),
            self.transitions
                .of_concrete_source
                .iter()
                .flat_map(move |(task_id, tr_ids)| {
                    tr_ids
                        .iter()
                        .map(move |&tr_id| (tr_id, (self.transitions.store[tr_id], Some(task_id))))
                }),
        )
    }

    /// Collect lifted and ground supports between transitions.
    /// Note that in the LP relaxation, effect transitions are allowed to
    /// be supporters of other effect transitions (on the same predicate / state function),
    /// which is not the case for causal links in the main encoding.
    /// In this specific case where the support is between two effects,
    /// the "active" literal is None (as this doesn't correspond to a causal link in the main CSP model).
    pub fn iter_supports(&self) -> impl Iterator<Item = ((TransitionId, TransitionId), Option<Lit>)> {
        // Supporting stemming from the original causal links in the main encoding.
        let supports_from_original_causal_links = self.main.causal_links.get_links().map(|cl| {
            let (tr1_id, _) = self.get_transition_of_effect(cl.eff_id).unwrap();
            let (tr2_id, _) = self.get_transition_of_condition(cl.cond_id).unwrap();
            debug_assert_eq!(
                self.get_transition_ref(tr1_id).get_state_var().fluent,
                self.get_transition_ref(tr2_id).get_state_var().fluent,
            );
            ((tr1_id, tr2_id), Some(cl.active))
        });

        // Supports from original (nondefault) effects to other original (nondefault) effects
        let supports_from_original_effects_to_others = self.iter_nondefault_effects().flat_map(move |(eff1_id, _)| {
            let (tr1_id, _) = self.get_transition_of_effect(eff1_id).unwrap();

            self.iter_nondefault_effects().flat_map(move |(eff2_id, _)| {
                let (tr2_id, tr2) = self.get_transition_of_effect(eff2_id).unwrap();
                if tr1_id == tr2_id || matches!(tr2, Transition::CondEff(_, _)) {
                    return None;
                }
                debug_assert!(matches!(tr2, Transition::Eff(_)));
                let (tr1_ref, tr2_ref) = (self.get_transition_ref(tr1_id), self.get_transition_ref(tr2_id));

                if tr2_ref.get_source().is_some() && tr1_ref.get_state_var().fluent == tr2_ref.get_state_var().fluent {
                    if tr1_ref
                        .get_args()
                        .iter()
                        .zip(tr2_ref.get_args().iter())
                        .any(|(term1, term2)| term1.is_cst() && term2.is_cst() && term1.cst() != term2.cst())
                    {
                        None
                    } else {
                        Some(((tr1_id, tr2_id), None))
                    }
                } else {
                    None
                }
            })
        });

        // Supports from default effects (ignored as non-necessary in the main encoding but required for the LP relaxation)
        let supports_from_default_effects = self.iter_default_effects().flat_map(move |(eff1_id, _)| {
            let (tr1_id, _) = self.get_transition_of_effect(eff1_id).unwrap();
            debug_assert!(self.get_transition_ref(tr1_id).iter_terms().all(|term| term.is_cst()));

            let to_conditions = self.iter_conditions().filter_map(move |(cond_id, _)| {
                let (tr2_id, _) = self.get_transition_of_condition(cond_id).unwrap();
                debug_assert!(tr1_id != tr2_id);

                let (tr1_ref, tr2_ref) = (self.get_transition_ref(tr1_id), self.get_transition_ref(tr2_id));

                if tr1_ref.get_state_var().fluent == tr2_ref.get_state_var().fluent {
                    if tr1_ref
                        .get_args()
                        .iter()
                        .zip(tr2_ref.get_args().iter())
                        .any(|(term1, term2)| term1.is_cst() && term2.is_cst() && term1.cst() != term2.cst())
                    {
                        return None;
                    }
                    if tr1_ref.get_valto().unwrap().is_cst()
                        && tr2_ref
                            .get_valfrom()
                            .map(|term| {
                                term.is_cst()
                                    && self.get_transition_ref(tr1_id).get_valto().unwrap().cst() != term.cst()
                            })
                            .unwrap_or_default()
                    {
                        return None;
                    }
                    Some(((tr1_id, tr2_id), None))
                } else {
                    None
                }
            });
            let to_original_effects = self.iter_nondefault_effects().flat_map(move |(eff2_id, _)| {
                let (tr2_id, tr2) = self.get_transition_of_effect(eff2_id).unwrap();
                if tr1_id == tr2_id || matches!(tr2, Transition::CondEff(_, _)) {
                    return None;
                }
                debug_assert!(matches!(tr2, Transition::Eff(_)));
                let (tr1_ref, tr2_ref) = (self.get_transition_ref(tr1_id), self.get_transition_ref(tr2_id));

                if tr2_ref.get_source().is_some() && tr1_ref.get_state_var().fluent == tr2_ref.get_state_var().fluent {
                    if tr1_ref
                        .get_args()
                        .iter()
                        .zip(tr2_ref.get_args().iter())
                        .any(|(term1, term2)| term1.is_cst() && term2.is_cst() && term1.cst() != term2.cst())
                    {
                        return None;
                    }
                    Some(((tr1_id, tr2_id), None))
                } else {
                    None
                }
            });
            std::iter::chain(to_conditions, to_original_effects)
        });

        {
            supports_from_original_causal_links
                .chain(supports_from_original_effects_to_others)
                .chain(supports_from_default_effects)
        }
        .inspect(|&((tr1_id, tr2_id), _)| {
            debug_assert!(
                tr1_id != tr2_id,
                "{:?} --- {:?}",
                self.get_transition_ref(tr1_id),
                self.get_transition_ref(tr2_id)
            );
            debug_assert!(!matches!(self.get_transition(tr1_id), Transition::Cond(_)));
            debug_assert!(
                !matches!(self.get_transition(tr2_id), Transition::Eff(_))
                    || self.get_transition_ref(tr2_id).get_source().is_some()
            );
        })
    }

    pub fn get_source(&self, source: &Source) -> Option<&Task> {
        source.map(|task_id| &self.main.sched.tasks[task_id])
    }
    pub fn get_source_terms(&self, source: &Source) -> &[IntTerm] {
        source
            .map(|task_id| &self.main.sched.tasks[task_id].args)
            .unwrap_or(&self.main.sched.global_args)
    }

    pub fn get_effect(&self, eff_id: EffectId) -> &Effect {
        if eff_id < self.transitions.first_default_initial_effect_id {
            self.main.sched.effects.get(eff_id)
        } else {
            &self.transitions.default_initial_effects[eff_id - self.transitions.first_default_initial_effect_id]
        }
    }
    pub fn get_condition(&self, cond_id: CondId) -> &HasValueAt {
        self.main.causal_links.conditions.get(cond_id)
    }

    pub fn get_transition(&self, transition_id: TransitionId) -> Transition {
        self.transitions.store[transition_id]
    }
    pub fn get_transition_ref(&'a self, transition_id: TransitionId) -> TransitionRef<'a> {
        match self.get_transition(transition_id) {
            Transition::Cond(c_id) => TransitionRef::Cond(self.get_condition(c_id)),
            Transition::Eff(e_id) => TransitionRef::Eff(self.get_effect(e_id)),
            Transition::CondEff(c_id, e_id) => TransitionRef::CondEff(self.get_condition(c_id), self.get_effect(e_id)),
        }
    }
    pub fn iter_transition_terms(&'a self, transition_id: TransitionId) -> impl Iterator<Item = &'a IntTerm> + 'a {
        self.get_transition_ref(transition_id)._iter_terms_move()
    }
    pub fn get_transition_of_condition(&'a self, condition_id: CondId) -> Option<(TransitionId, Transition)> {
        self.transitions
            .of_condition
            .get(condition_id)
            .map(|&tr_id| (tr_id, self.transitions.store[tr_id]))
    }
    pub fn get_transition_of_effect(&'a self, effect_id: EffectId) -> Option<(TransitionId, Transition)> {
        self.transitions
            .of_effect
            .get(effect_id)
            .map(|&tr_id| (tr_id, self.transitions.store[tr_id]))
    }
    pub fn get_transitions_of_source(&self, source: &Source) -> impl Iterator<Item = (TransitionId, Transition)> {
        match source {
            None => Some(self.transitions.of_empty_source.iter()),
            Some(task_id) => self.transitions.of_concrete_source.get(task_id).map(|v| v.iter()),
        }
        .unwrap_or([].iter())
        .map(|&tr_id| (tr_id, self.transitions.store[tr_id]))
    }

    /*#[allow(dead_code)]
    pub fn get_transitions_of_source_conditions(&self, source: &Source) -> impl Iterator<Item = (TransitionId, Transition)> {
        match source {
            None => Some(self.transitions.of_empty_source.iter()),
            Some(task_id) => self.transitions.of_concrete_source.get(task_id).map(|v| v.iter()),
        }
        .unwrap_or([].iter())
        .map(|&tr_id| (tr_id, self.transitions.store[tr_id]))
        .filter(|(_, tr)| matches!(tr, Transition::Cond(_) | Transition::CondEff(_, _)))
    }
    #[allow(dead_code)]
    pub fn get_transitions_of_source_effects(&self, source: &Source) -> impl Iterator<Item = (TransitionId, Transition)> {
        match source {
            None => Some(self.transitions.of_empty_source.iter()),
            Some(task_id) => self.transitions.of_concrete_source.get(task_id).map(|v| v.iter()),
        }
        .unwrap_or([].iter())
        .map(|&tr_id| (tr_id, self.transitions.store[tr_id]))
        .filter(|&(_, tr)| matches!(tr, Transition::Eff(_) | Transition::CondEff(_, _)))
    }*/

    pub fn get_source_groundings(&self, source: Source) -> (Vec<SourceGrounding>, bool) {
        // let ranges = source
        //     .map(|task_id| self.concrete_sources_terms_ranges[task_id.to_int() as usize].as_slice())
        //     .unwrap_or(self.empty_source_terms_ranges.as_slice());
        // let groundings = Vec::from_iter(
        //     ranges
        //         .iter()
        //         .map(|&(lb, ub)| lb..=ub)
        //         .multi_cartesian_product()
        //         .map(SourceGrounding::from),
        // );
        let groundings = self
            .get_source_terms(&source)
            .iter()
            .map(|t| self.main.bounds(t).0..=self.main.bounds(t).1)
            .multi_cartesian_product()
            .map(SourceGrounding::from)
            .collect();
        let complete = true;
        (groundings, complete)
    }
    pub fn get_transition_groundings(&self, transition_id: TransitionId) -> Vec<TransitionGrounding> {
        // self.transitions_terms_ranges[transition_id]
        //     .iter()
        //     .map(|&(lb, ub)| lb..=ub)
        //     .multi_cartesian_product()
        //     .map(TransitionGrounding::from)
        //     .collect()
        self.iter_transition_terms(transition_id)
            .map(|t| self.main.bounds(t).0..=self.main.bounds(t).1)
            .multi_cartesian_product()
            .map(TransitionGrounding::from)
            .collect()
    }

    pub fn flatten_source_grounding(&self, source: Source, grounding: &SourceGrounding) -> SourceGroundingFlatId {
        grounding.to_flat_id(
            source
                .map(|task_id| self.concrete_sources_terms_ranges[task_id.to_int() as usize].as_slice())
                .unwrap_or(self.empty_source_terms_ranges.as_slice()),
        )
    }
    pub fn flatten_transition_grounding(
        &self,
        transition_id: TransitionId,
        grounding: &TransitionGrounding,
    ) -> TransitionGroundingFlatId {
        grounding.to_flat_id(self.transitions_terms_ranges[transition_id].as_slice())
    }

    pub fn build_transition_grounding_from_source_grounding(
        &self,
        transition_id: TransitionId,
        source_grounding: &SourceGrounding,
    ) -> TransitionGrounding {
        // Returns the position of the transition's terms within the source's terms
        // (i.e. the collection of all terms appearing in the source's transitions).
        // None corresponds to a constant term.
        let get_transition_terms_positions_in_source_terms = |transition_id: TransitionId| -> &[Option<usize>] {
            self.transitions.transition_terms_indices_in_source[transition_id].as_slice()
        };
        let transition_ref = self.get_transition_ref(transition_id);
        TransitionGrounding::from(
            get_transition_terms_positions_in_source_terms(transition_id)
                .iter()
                .enumerate()
                .map(|(i, j)| {
                    if let Some(j) = j {
                        source_grounding.inner()[*j]
                    } else {
                        debug_assert!(transition_ref.get_term(i).is_cst());
                        transition_ref.get_term(i).cst()
                    }
                })
                .collect(),
        )
    }
}

pub(crate) struct Transitions {
    pub store: Vec<Transition>,
    pub transition_terms_indices_in_source: Vec<SmallVec<[Option<usize>; 6]>>,

    pub of_condition: DirectIdMap<CondId, TransitionId>,
    pub of_effect: DirectIdMap<EffectId, TransitionId>,
    pub of_empty_source: Vec<TransitionId>,
    pub of_concrete_source: DirectIdMap<TaskId, Vec<TransitionId>>,

    pub default_initial_effects: Vec<Effect>,
    pub first_default_initial_effect_id: EffectId,
}

impl Transitions {
    /// Collects transitions from "unambiguous" conditions and effects
    /// (i.e. those whose terms are constants or arguments of their source (task), meaning,
    /// for example that a condition using a reified variable as a term will be ignored).
    ///
    pub fn new_unambiguous(ctx: &mut SchedEncoder) -> Self {
        // Collects ambiguous / unsupported transitions to ignore / relax.
        let (conditions_to_ignore, effects_to_ignore) = collect_ambiguous_conditions_and_effects_to_relax(ctx);

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

        let mut default_effects_ground_args = HashMap::<Sym, HashSet<Vec<IntCst>>>::new();

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
                let tr_id = store.len();
                of_condition.insert(cond_id, tr_id);
                if let Some(task_id) = src {
                    if !of_concrete_source.contains_key(task_id) {
                        of_concrete_source.insert(task_id, vec![]);
                    }
                    of_concrete_source.get_mut(task_id).unwrap().push(tr_id);
                } else {
                    of_empty_source.push(tr_id);
                }
                store.push(Transition::Cond(cond_id));
            }
        }
        for (src, es) in source_effs_iter {
            for &(eff_id, e) in es {
                let mut compatible_conds_found = 0;

                // No CondEff pattern allowed for empty source.
                if src.is_some() {
                    let cs = if let Some(task_id) = src {
                        concrete_source_conditions.get(task_id)
                    } else {
                        Some(&empty_source_conditions)
                    }
                    .into_iter()
                    .flatten();

                    for &(cond_id, c) in cs {
                        if e.state_var == c.state_var && e.prez == c.prez {
                            // Change the previously inserted Cond transition into a CondEff
                            let tr_id = *of_condition.get(cond_id).unwrap();
                            of_effect.insert(eff_id, tr_id);
                            store[tr_id] = Transition::CondEff(cond_id, eff_id);

                            compatible_conds_found += 1;
                        }
                    }
                    debug_assert!(compatible_conds_found <= 1);
                }

                // Add a new Eff transition if the effect doesn't correspond to a CondEff
                if compatible_conds_found == 0 {
                    let tr_id = store.len();
                    of_effect.insert(eff_id, tr_id);
                    if let Some(task_id) = src {
                        if !of_concrete_source.contains_key(task_id) {
                            of_concrete_source.insert(task_id, vec![]);
                        }
                        of_concrete_source.get_mut(task_id).unwrap().push(tr_id);
                    } else {
                        of_empty_source.push(tr_id);
                    }
                    store.push(Transition::Eff(eff_id));
                }

                // Remember the args groundings of ground initial effects
                if src.is_none() && e.state_var.args.iter().all(|term| term.is_cst()) {
                    let ground_args = e.state_var.args.iter().map(|term| term.cst()).collect();
                    default_effects_ground_args
                        .entry(e.state_var.fluent.to_string())
                        .or_default()
                        .insert(ground_args);
                }
            }
        }

        // Loop over fluents and their parameter types' ground values.
        // For each such grounding, introduce an initial effect (with default value (0)),
        // if there wasn't already an effect with the same ground parameters encountered earlier
        // (among the "explicit" known initial effects accessible from `ctx`).

        let mut default_initial_effects = vec![];
        let first_default_initial_effect_id = of_effect.len();

        for (sym, params, _) in ctx.sched.fluents.iter() {
            let t = crate::Time::from(-2);
            let args = crate::boxes::BBox::new(params.iter().map(|p| p.range).collect::<Vec<_>>());
            let mut grs = args.as_ref().points();
            while let Some(gr) = grs.next() {
                let args_ground = Vec::from_iter(gr.iter().copied());

                // Ignore if this there already is an initial effect with these ground args.
                if default_effects_ground_args
                    .get(sym)
                    .is_some_and(|known_grs| known_grs.contains(&args_ground))
                {
                    continue;
                }
                let args_ground = args_ground.into_iter().map(IntTerm::int_cst).collect();

                default_initial_effects.push(Effect {
                    transition_start: t,
                    transition_end: t,
                    mutex_end: ctx.store.new_ivar(-2, INT_CST_MAX, "_").into(),
                    state_var: StateVar {
                        fluent: sym.to_string(),
                        args: args_ground,
                    },
                    operation: crate::EffectOp::Assign(IntTerm::ZERO),
                    prez: Lit::TRUE,
                    source: None,
                });

                let eff_id = of_effect.len();
                let tr_id = store.len();
                of_effect.insert(eff_id, tr_id);
                of_empty_source.push(tr_id);
                store.push(Transition::Eff(eff_id));
            }
        }
        debug_assert!(default_initial_effects.iter().all(|e| {
            e.state_var
                .args
                .iter()
                .chain(match &e.operation {
                    crate::EffectOp::Assign(term) => [term],
                    crate::EffectOp::Step(_term) => todo!(),
                })
                .all(|term| term.is_cst())
        }));

        // For each transition, collect its terms' (args and values) indices in the list of its source's args.
        //
        // Note that currently, transitions whose terms contain auxiliary or reification variables
        // that do not appearing in the the source's args are ignored anyway (filtered out as ambiguous)

        let mut transition_terms_indices_in_source = Vec::with_capacity(store.len());

        let get_source_terms = |src| {
            if let Some(task_id) = src {
                ctx.sched.tasks[task_id].args.as_slice()
            } else {
                ctx.sched.global_args.as_slice()
            }
        };

        for &tr_id in store.iter() {
            let entry = match tr_id {
                Transition::Cond(c_id) => {
                    let c = ctx.causal_links.conditions.get(c_id);
                    let src_terms = get_source_terms(c.source);
                    c.state_var
                        .args
                        .iter()
                        .chain(&[c.value])
                        .map(|&term| {
                            (!term.is_cst())
                                .then(|| src_terms.iter().position(|&t| t == term))
                                .flatten()
                        })
                        .collect()
                }
                Transition::Eff(e_id) => {
                    let e = if e_id < first_default_initial_effect_id {
                        ctx.sched.effects.get(e_id)
                    } else {
                        default_initial_effects
                            .get(e_id - first_default_initial_effect_id)
                            .unwrap()
                    };
                    let src_terms = get_source_terms(e.source);
                    e.state_var
                        .args
                        .iter()
                        .chain(match &e.operation {
                            crate::EffectOp::Assign(term) => [term],
                            crate::EffectOp::Step(_term) => todo!(),
                        })
                        .map(|&term| {
                            (!term.is_cst())
                                .then(|| src_terms.iter().position(|&t| t == term))
                                .flatten()
                        })
                        .collect()
                }
                Transition::CondEff(c_id, e_id) => {
                    let c = ctx.causal_links.conditions.get(c_id);
                    let e = if e_id < first_default_initial_effect_id {
                        ctx.sched.effects.get(e_id)
                    } else {
                        default_initial_effects
                            .get(e_id - first_default_initial_effect_id)
                            .unwrap()
                    };
                    debug_assert!(e.source == c.source);
                    let src_terms = get_source_terms(c.source);
                    c.state_var
                        .args
                        .iter()
                        .chain([&c.value])
                        .chain(match &e.operation {
                            crate::EffectOp::Assign(term) => [term],
                            crate::EffectOp::Step(_term) => todo!(),
                        })
                        .map(|&term| {
                            (!term.is_cst())
                                .then(|| src_terms.iter().position(|&t| t == term))
                                .flatten()
                        })
                        .collect()
                }
            };
            transition_terms_indices_in_source.push(entry);
        }

        Self {
            store,
            transition_terms_indices_in_source,
            of_condition,
            of_effect,
            of_empty_source,
            of_concrete_source,
            default_initial_effects,
            first_default_initial_effect_id,
        }
    }
}
