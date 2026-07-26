use std::collections::{HashMap, HashSet};

use aries_solver::prelude::*;
use idmap::{DirectIdMap};
use smallvec::SmallVec;
use streaming_iterator::StreamingIterator;

use crate::encoder::{CondId, SchedEncoder};
use crate::ext::lprelax::transition::*;
use crate::ext::{collect_ambiguous_conditions_and_effects_to_relax};
use crate::{Effect, EffectId, IntTerm, StateVar, Sym, TaskId};

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
                    let ground_args = e.state_var.args.iter().map(|term| term.constant).collect();
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
        let first_default_initial_effect_id = 1 + of_effect.iter().max_by_key(|&(eff_id, _)| eff_id).unwrap().0;

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
                    operation: crate::EffectOp::Assign(IntTerm::int_cst(
                        ctx.sched.fluents.get_return(sym).unwrap().range.first,
                    )),
                    prez: Lit::TRUE,
                    source: None,
                });

                let eff_id = first_default_initial_effect_id + default_initial_effects.len() - 1;
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
                                .then(|| src_terms.iter().position(|&(t, _)| t == term))
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
                                .then(|| src_terms.iter().position(|&(t, _)| t == term))
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
                                .then(|| src_terms.iter().position(|&(t, _)| t == term))
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
