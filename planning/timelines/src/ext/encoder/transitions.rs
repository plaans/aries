use std::collections::{BTreeMap, BTreeSet, HashSet};

use aries::prelude::*;
use aries::utils::StreamingIterator;
use idmap::DirectIdMap;
use smallvec::SmallVec;

use crate::encoder::{CondId, SchedEncoder};
use crate::ext::transition::TransitionId;
use crate::{Effect, EffectId, IntTerm, StateVar, Sym, TaskId};

type TransitionIndex = usize;

pub(crate) struct Transitions {
    pub store: Vec<TransitionId>,

    pub of_condition: DirectIdMap<CondId, TransitionIndex>,
    pub of_effect: DirectIdMap<EffectId, TransitionIndex>,
    pub of_empty_source: Vec<TransitionIndex>,
    pub of_concrete_source: DirectIdMap<TaskId, SmallVec<[TransitionIndex; 6]>>,

    pub default_initial_effects: Vec<Effect>,
    pub first_default_initial_effect_id: EffectId,

    pub empty_source_transitions_terms: Vec<IntTerm>,
    pub concrete_source_transitions_terms: DirectIdMap<TaskId, Vec<IntTerm>>,
    /// For each transition, stores the indices of its (non-constant) terms in
    /// the collection (vector) of all terms appearing in this and and "sibling" transitions,
    /// to prevent from recomputing them too often later.
    pub transition_terms_indices_in_source: BTreeMap<TransitionId, SmallVec<[Option<usize>; 6]>>,
}

impl Transitions {
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
                let tr_index = store.len();
                of_condition.insert(cond_id, tr_index);
                if let Some(task_id) = src {
                    if !of_concrete_source.contains_key(task_id) {
                        of_concrete_source.insert(task_id, SmallVec::new());
                    }
                    of_concrete_source.get_mut(task_id).unwrap().push(tr_index);
                } else {
                    of_empty_source.push(tr_index);
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
                            let tr_index = *of_condition.get(cond_id).unwrap();
                            of_effect.insert(eff_id, tr_index);
                            store[tr_index] = TransitionId::CondEff(cond_id, eff_id);

                            compatible_conds_found += 1;
                        }
                    }
                    debug_assert!(compatible_conds_found <= 1);
                }

                // Add a new Eff transition if the effect doesn't correspond to a CondEff
                if compatible_conds_found == 0 {
                    let tr_index = store.len();
                    of_effect.insert(eff_id, tr_index);
                    if let Some(task_id) = src {
                        if !of_concrete_source.contains_key(task_id) {
                            of_concrete_source.insert(task_id, SmallVec::new());
                        }
                        of_concrete_source.get_mut(task_id).unwrap().push(tr_index);
                    } else {
                        of_empty_source.push(tr_index);
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
                let tr_index = store.len();
                of_effect.insert(eff_id, tr_index);
                of_empty_source.push(tr_index);
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
