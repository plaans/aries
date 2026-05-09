use std::collections::{BTreeMap, BTreeSet, HashSet};

use aries::core::{INT_CST_MAX, IntCst};
use aries::prelude::Lit;
use aries::utils::StreamingIterator;
use idmap::DirectIdMap;
use smallvec::SmallVec;

use crate::constraints::HasValueAt;
use crate::encoder::{CondId, SchedEncoder};
use crate::ext::{SchedEncoderExt, Source};
use crate::{Effect, EffectId, IntTerm, StateVar, Sym, TaskId};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitionId {
    Cond(CondId),
    Eff(EffectId),
    /// A condition and effect sharing the same source, presence literal, and state variable.
    CondEff(CondId, EffectId),
}

impl TransitionId {
    pub fn as_ref<'a>(self, ctx: &'a SchedEncoderExt) -> TransitionRef<'a> {
        match self {
            TransitionId::Cond(c_id) => TransitionRef::Cond(ctx.get_condition(c_id)),
            TransitionId::Eff(e_id) => TransitionRef::Eff(ctx.get_effect(e_id)),
            TransitionId::CondEff(c_id, e_id) => TransitionRef::CondEff(ctx.get_condition(c_id), ctx.get_effect(e_id)),
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum TransitionRef<'a> {
    Cond(&'a HasValueAt),
    Eff(&'a Effect),
    CondEff(&'a HasValueAt, &'a Effect),
}
impl<'a> TransitionRef<'a> {
    pub fn get_condition(&self) -> Option<&'a HasValueAt> {
        match self {
            TransitionRef::Eff(_) => None,
            TransitionRef::Cond(c) | TransitionRef::CondEff(c, _) => Some(c),
        }
    }
    pub fn get_effect(&self) -> Option<&'a Effect> {
        match self {
            TransitionRef::Cond(_) => None,
            TransitionRef::Eff(e) | TransitionRef::CondEff(_, e) => Some(e),
        }
    }
    pub fn get_source(&self) -> Source {
        match self {
            TransitionRef::Cond(c) => c.source,
            TransitionRef::Eff(e) => e.source,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.source == e.source);
                c.source
            }
        }
    }
    pub fn get_prez(&self) -> Lit {
        match self {
            TransitionRef::Cond(c) => c.prez,
            TransitionRef::Eff(e) => e.prez,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.prez == e.prez);
                c.prez
            }
        }
    }
    pub fn get_state_var(&self) -> &'a StateVar {
        match self {
            TransitionRef::Cond(c) => &c.state_var,
            TransitionRef::Eff(e) => &e.state_var,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.state_var == e.state_var);
                &c.state_var
            }
        }
    }
    pub fn get_args(&self) -> &'a [IntTerm] {
        &self.get_state_var().args
    }
    pub fn get_valfrom(&self) -> Option<&IntTerm> {
        self.get_condition().map(|c| &c.value)
    }
    pub fn get_valto(&self) -> Option<&IntTerm> {
        self.get_effect().map(|e| match &e.operation {
            crate::EffectOp::Assign(term) => term,
            crate::EffectOp::Step(_) => todo!(),
        })
    }
    pub fn get_terms(&self) -> (&'a [IntTerm], Option<&IntTerm>, Option<&IntTerm>) {
        debug_assert!(self.get_valfrom().is_some() || self.get_valto().is_some());
        (self.get_args(), self.get_valfrom(), self.get_valto())
    }
    pub fn terms_len(&self) -> usize {
        self.get_args().len() + self.get_valfrom().is_some() as usize + self.get_valto().is_some() as usize
    }
    pub fn get_term(&self, i: usize) -> &IntTerm {
        if i == self.get_args().len() + 1 {
            self.get_valto().unwrap()
        } else if i == self.get_args().len() {
            self.get_valfrom().unwrap()
        } else {
            &self.get_args()[i]
        }
    }
    pub fn get_valfrom_idx(&self) -> Option<usize> {
        match self {
            TransitionRef::Cond(_) => Some(self.get_args().len()),
            TransitionRef::Eff(_) => None,
            TransitionRef::CondEff(_, _) => Some(self.get_args().len()),
        }
    }
    pub fn get_valto_idx(&self) -> Option<usize> {
        match self {
            TransitionRef::Cond(_) => None,
            TransitionRef::Eff(_) => Some(self.get_args().len()),
            TransitionRef::CondEff(_, _) => Some(self.get_args().len() + 1),
        }
    }
    /*pub fn terms_iter(&self) -> impl Iterator<Item = &IntTerm> {
        self.args().iter().chain(self.valfrom()).chain(self.valto())
    }*/
    pub fn iter_terms(&self) -> impl Iterator<Item = IntTerm> + use<'_> {
        self.get_args()
            .iter()
            .copied()
            .chain(self.get_valfrom().copied())
            .chain(self.get_valto().copied())
    }
}

impl<'a> std::fmt::Debug for TransitionRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}]({:?}: {:?}->{:?})",
            self.get_source(),
            self.get_state_var(),
            self.get_valfrom(),
            self.get_valto(),
        )
    }
}

type TransitionIndex = usize;

pub struct Transitions {
    store: Vec<TransitionId>,

    of_condition: DirectIdMap<CondId, TransitionIndex>,
    of_effect: DirectIdMap<EffectId, TransitionIndex>,
    of_empty_source: Vec<TransitionIndex>,
    of_concrete_source: DirectIdMap<TaskId, SmallVec<[TransitionIndex; 6]>>,

    default_initial_effects: Vec<Effect>,
    first_default_initial_effect_id: EffectId,

    empty_source_transitions_terms: Vec<IntTerm>,
    concrete_source_transitions_terms: DirectIdMap<TaskId, Vec<IntTerm>>,
    /// For each transition, stores the indices of its (non-constant) terms in
    /// the collection (vector) of all terms appearing in this and and "sibling" transitions,
    /// to prevent from recomputing them too often later.
    transition_terms_indices_in_source: BTreeMap<TransitionId, SmallVec<[Option<usize>; 6]>>,
}

impl Transitions {
    pub fn get_default_initial_effects(&self) -> (EffectId, &[Effect]) {
        (self.first_default_initial_effect_id, &self.default_initial_effects)
    }
    pub fn iter_default_initial_effects(&self) -> impl Iterator<Item = (EffectId, &Effect)> {
        self.default_initial_effects
            .iter()
            .enumerate()
            .map(|(i, e)| (self.first_default_initial_effect_id + i, e))
    }

    pub fn iter(&self) -> impl Iterator<Item = (TransitionId, Source)> {
        std::iter::chain(
            self.of_empty_source.iter().map(|tr_id| (self.store[*tr_id], None)),
            self.of_concrete_source
                .iter()
                .flat_map(move |(task_id, tr_ids)| tr_ids.iter().map(move |tr_id| (self.store[*tr_id], Some(task_id)))),
        )
    }

    pub fn get_for_condition(&self, condition_id: CondId) -> Option<&TransitionId> {
        self.of_condition.get(condition_id).map(|&tr_id| &self.store[tr_id])
    }
    pub fn get_for_effect(&self, effect_id: EffectId) -> Option<&TransitionId> {
        self.of_effect.get(effect_id).map(|&tr_id| &self.store[tr_id])
    }
    pub fn get_for_source(&self, source: &Source) -> impl Iterator<Item = &TransitionId> {
        match source {
            None => Some(self.of_empty_source.iter()),
            Some(task_id) => self.of_concrete_source.get(task_id).map(|v| v.iter()),
        }
        .unwrap_or([].iter())
        .map(|&tr_id| &self.store[tr_id])
    }

    pub fn get_source_terms(&self, source: &Source) -> &[IntTerm] {
        source
            .map(|task_id| &self.concrete_source_transitions_terms[task_id])
            .unwrap_or(&self.empty_source_transitions_terms)
    }

    /// Returns the position of the transition's terms within the source's terms
    /// (i.e. the collection of all terms appearing in the source's transitions).
    /// None corresponds to a constant term.
    pub fn get_transition_terms_positions_in_source_terms(
        &self,
        transition_id: &TransitionId,
    ) -> Option<impl Iterator<Item = Option<usize>>> {
        self.transition_terms_indices_in_source
            .get(transition_id)
            .map(|v| v.iter().copied())
    }

    pub fn new(ctx: &mut SchedEncoder) -> Self {
        let mut store = vec![];
        let mut of_condition = DirectIdMap::default();
        let mut of_effect = DirectIdMap::default();
        let mut of_empty_source = vec![];
        let mut of_concrete_source = DirectIdMap::default();

        let mut empty_source_conds = vec![];
        let mut concrete_source_conds = DirectIdMap::default();
        for (cond_id, c) in ctx.causal_links.conditions.iter().enumerate() {
            if let Some(task_id) = c.source {
                if !concrete_source_conds.contains_key(task_id) {
                    concrete_source_conds.insert(task_id, vec![]);
                }
                concrete_source_conds.get_mut(task_id).unwrap().push((cond_id, c));
            } else {
                empty_source_conds.push((cond_id, c));
            }
        }

        let mut empty_source_effs = vec![];
        let mut concrete_source_effs = DirectIdMap::default();
        for (eff_id, e) in ctx.sched.effects.iter().enumerate() {
            if let Some(task_id) = e.source {
                if !concrete_source_effs.contains_key(task_id) {
                    concrete_source_effs.insert(task_id, vec![]);
                }
                concrete_source_effs.get_mut(task_id).unwrap().push((eff_id, e));
            } else {
                empty_source_effs.push((eff_id, e));
            }
        }

        let mut initial_effs_ground_args = BTreeMap::<Sym, HashSet<Vec<IntCst>>>::new();

        // First, iterate over conditions (grouped by sources) and introduce corresponding Cond transitions.
        // Then, iterate over effects (grouped by sources) and the conditions for those sources.
        // When a compatible condition and effect are found, a corresponding CondEff transition is introduced,
        // modifying the previously inserted Cond transition.
        // If no compatible condition is found, a Eff transition is introduced.
        //
        // If a ground initial (empty source) Eff transition is introduced, remember that grounding.
        // This is needed to avoid overriding it later when introducing the default (negative) ground initial effects.

        let source_conds_iter = std::iter::chain(
            [(None, &empty_source_conds)],
            concrete_source_conds
                .iter()
                .map(|(task_id, conds)| (Some(task_id), conds)),
        );
        for (src, cs) in source_conds_iter {
            for &(cond_id, _) in cs {
                let tr_id = store.len();
                of_condition.insert(cond_id, tr_id);
                if let Some(task_id) = src {
                    if !of_concrete_source.contains_key(task_id) {
                        of_concrete_source.insert(task_id, SmallVec::new());
                    }
                    of_concrete_source.get_mut(task_id).unwrap().push(tr_id);
                } else {
                    of_empty_source.push(tr_id);
                }
                store.push(TransitionId::Cond(cond_id));
            }
        }

        let source_effs_iter = std::iter::chain(
            [(None, &empty_source_effs)],
            concrete_source_effs.iter().map(|(task_id, effs)| (Some(task_id), effs)),
        );
        for (src, es) in source_effs_iter {
            for &(eff_id, e) in es {
                let mut compatible_conds_found = 0;

                // No CondEff pattern allowed for empty source.
                if src.is_some() {
                    let cs = if let Some(task_id) = src {
                        concrete_source_conds.get(task_id)
                    } else {
                        Some(&empty_source_conds)
                    }
                    .into_iter()
                    .flatten();

                    for &(cond_id, c) in cs {
                        if e.state_var == c.state_var && e.prez == c.prez {
                            // Change the previously inserted Cond transition into a CondEff
                            let tr_id = *of_condition.get(cond_id).unwrap();
                            of_effect.insert(eff_id, tr_id);
                            store[tr_id] = TransitionId::CondEff(cond_id, eff_id);

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
                            of_concrete_source.insert(task_id, SmallVec::new());
                        }
                        of_concrete_source.get_mut(task_id).unwrap().push(tr_id);
                    } else {
                        of_empty_source.push(tr_id);
                    }
                    store.push(TransitionId::Eff(eff_id));
                }

                // Remember the args groundings of ground initial effects
                if src.is_none() && e.state_var.args.iter().all(|term| term.is_cst()) {
                    let ground_args = e.state_var.args.iter().map(|term| term.cst()).collect();
                    initial_effs_ground_args
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
                if initial_effs_ground_args
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
                store.push(TransitionId::Eff(eff_id));
            }
        }
        debug_assert!(default_initial_effects.iter().all(|e| {
            e.state_var
                .args
                .iter()
                .chain(match &e.operation {
                    crate::EffectOp::Assign(term) => [term],
                    crate::EffectOp::Step(term) => todo!(),
                })
                .all(|term| term.is_cst())
        }));

        // First, collect the (non-constant) terms appearing in all transitions associated to each source.
        //
        // Then, for each transition, collect its terms' (args and values) indices in this set (for its source).
        // A None index corresponds to a constant term.

        let empty_source_transitions_terms = {
            let mut res = BTreeSet::<IntTerm>::new();

            for (_, c) in empty_source_conds {
                res.extend(c.state_var.args.iter().chain(&[c.value]).filter(|term| !term.is_cst()));
            }
            for (_, e) in empty_source_effs {
                res.extend(
                    e.state_var
                        .args
                        .iter()
                        .chain(match &e.operation {
                            crate::EffectOp::Assign(term) => [term],
                            crate::EffectOp::Step(term) => todo!(),
                        })
                        .filter(|term| !term.is_cst()),
                );
            }
            Vec::from_iter(res)
        };

        let concrete_source_transitions_terms = {
            let mut res = DirectIdMap::<TaskId, BTreeSet<IntTerm>>::new();

            for (task_id, cs) in concrete_source_conds.iter() {
                if !res.contains_key(task_id) {
                    res.insert(task_id, BTreeSet::new());
                }
                res.get_mut(task_id).unwrap().extend(
                    cs.iter()
                        .flat_map(|(_, c)| c.state_var.args.iter().chain([&c.value]).filter(|term| !term.is_cst())),
                );
            }
            for (task_id, es) in concrete_source_effs.iter() {
                if !res.contains_key(task_id) {
                    res.insert(task_id, BTreeSet::new());
                }
                res.get_mut(task_id).unwrap().extend(es.iter().flat_map(|(_, e)| {
                    e.state_var
                        .args
                        .iter()
                        .chain(match &e.operation {
                            crate::EffectOp::Assign(term) => [term],
                            crate::EffectOp::Step(term) => todo!(),
                        })
                        .filter(|term| !term.is_cst())
                }));
            }
            DirectIdMap::from_iter(res.into_iter().map(|(task_id, set)| (task_id, Vec::from_iter(set))))
        };

        let mut transition_terms_indices_in_source: BTreeMap<TransitionId, SmallVec<[Option<usize>; 6]>> =
            BTreeMap::new();

        for &tr_id in store.iter() {
            let entry = match tr_id {
                TransitionId::Cond(c_id) => {
                    let c = ctx.causal_links.conditions.get(c_id);
                    let src_terms = if let Some(task_id) = c.source {
                        concrete_source_transitions_terms.get(task_id).unwrap().as_slice()
                    } else {
                        empty_source_transitions_terms.as_slice()
                    };
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
                TransitionId::Eff(e_id) => {
                    let e = if e_id < first_default_initial_effect_id {
                        ctx.sched.effects.get(e_id)
                    } else {
                        default_initial_effects
                            .get(e_id - first_default_initial_effect_id)
                            .unwrap()
                    };
                    let src_terms = if let Some(task_id) = e.source {
                        concrete_source_transitions_terms.get(task_id).unwrap().as_slice()
                    } else {
                        empty_source_transitions_terms.as_slice()
                    };
                    e.state_var
                        .args
                        .iter()
                        .chain(match &e.operation {
                            crate::EffectOp::Assign(term) => [term],
                            crate::EffectOp::Step(term) => todo!(),
                        })
                        .map(|&term| {
                            (!term.is_cst())
                                .then(|| src_terms.iter().position(|&t| t == term))
                                .flatten()
                        })
                        .collect()
                }
                TransitionId::CondEff(c_id, e_id) => {
                    let c = ctx.causal_links.conditions.get(c_id);
                    let e = if e_id < first_default_initial_effect_id {
                        ctx.sched.effects.get(e_id)
                    } else {
                        default_initial_effects
                            .get(e_id - first_default_initial_effect_id)
                            .unwrap()
                    };
                    debug_assert!(e.source == c.source);
                    let src_terms = if let Some(task_id) = c.source {
                        concrete_source_transitions_terms.get(task_id).unwrap().as_slice()
                    } else {
                        empty_source_transitions_terms.as_slice()
                    };
                    c.state_var
                        .args
                        .iter()
                        .chain([&c.value])
                        .chain(match &e.operation {
                            crate::EffectOp::Assign(term) => [term],
                            crate::EffectOp::Step(term) => todo!(),
                        })
                        .map(|&term| {
                            (!term.is_cst())
                                .then(|| src_terms.iter().position(|&t| t == term))
                                .flatten()
                        })
                        .collect()
                }
            };
            transition_terms_indices_in_source.insert(tr_id, entry);
        }

        Self {
            store,
            of_condition,
            of_effect,
            of_empty_source,
            of_concrete_source,
            default_initial_effects,
            first_default_initial_effect_id,
            empty_source_transitions_terms,
            concrete_source_transitions_terms,
            transition_terms_indices_in_source,
        }
    }
}
