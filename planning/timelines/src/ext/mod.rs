pub mod ground;
pub mod lprelax;
pub mod transitions;

use std::collections::BTreeSet;

use crate::boxes::BBox;
use crate::constraints::HasValueAt;
use crate::encoder::CondId;
use crate::{Effect, EffectId, StateVar, Task, TaskId, Time};
use crate::{IntTerm, encoder::SchedEncoder};

use aries::core::views::Dom;
use aries::model::lang::ModelWrapper;
use aries::prelude::*;

use aries::utils::StreamingIterator;
use idmap::DirectIdMap;
use itertools::Itertools;
pub(crate) use transitions::*;

type Source = Option<TaskId>;

pub struct SchedEncoderExt<'a> {
    pub(crate) main: &'a mut SchedEncoder,

    pub(crate) transitions: Transitions,

    empty_source_transitions_terms: Vec<IntTerm>,
    concrete_source_transitions_terms: DirectIdMap<TaskId, Vec<IntTerm>>,
    default_initial_effects: Vec<Effect>,
    _default_initial_effects_starting_id: EffectId,

    pub(crate) lprelax: Option<aries_lprelax::LpRelax>,
}

impl<'a> Dom for SchedEncoderExt<'a> {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.main.upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.main.presence(var)
    }
}
impl<'a> ModelWrapper for SchedEncoderExt<'a> {
    type Lbl = String;

    fn get_model(&self) -> &crate::Model {
        self.main.get_model()
    }

    fn get_model_mut(&mut self) -> &mut crate::Model {
        self.main.get_model_mut()
    }
}

impl<'a> SchedEncoderExt<'a> {
    pub fn get_effect(&self, eff_id: EffectId) -> &Effect {
        let n = self.main.sched.effects.iter().try_len().unwrap();
        if eff_id < n {
            self.main.sched.effects.get(eff_id)
        } else {
            &self.default_initial_effects[eff_id - n]
        }
    }

    pub fn get_condition(&self, cond_id: CondId) -> &HasValueAt {
        self.main.causal_links.conditions.get(cond_id)
    }

    pub fn get_source(&self, source: &Source) -> Option<&Task> {
        source.map(|task_id| &self.main.sched.tasks[task_id])
    }

    pub fn get_source_terms(&self, source: &Source) -> &[IntTerm] {
        source
            .map(|task_id| &self.concrete_source_transitions_terms[task_id])
            .unwrap_or(&self.empty_source_transitions_terms)
    }

    pub fn new(sched_encoder: &'a mut SchedEncoder) -> Self {
        let (concrete_source_transitions_terms, _default_initial_effects_starting_id) = {
            let mut res = DirectIdMap::<TaskId, BTreeSet<IntTerm>>::new();
            let mut n = 0;
            for e in sched_encoder.sched.effects.iter() {
                n += 1;
                if let Some(task_id) = e.source {
                    if !res.contains_key(task_id) {
                        res.insert(task_id, BTreeSet::new());
                    }
                    res.get_mut(task_id)
                        .unwrap()
                        .extend(e.state_var.args.iter().filter(|term| !term.is_cst()));
                }
            }
            for c in sched_encoder.causal_links.conditions.iter() {
                if let Some(task_id) = c.source {
                    if !res.contains_key(task_id) {
                        res.insert(task_id, BTreeSet::new());
                    }
                    res.get_mut(task_id)
                        .unwrap()
                        .extend(c.state_var.args.iter().filter(|term| !term.is_cst()));
                }
            }
            (
                DirectIdMap::from_iter(res.into_iter().map(|(task_id, set)| (task_id, Vec::from_iter(set)))),
                n,
            )
        };

        let default_initial_effects = {
            let mut res = vec![];
            for (sym, params) in sched_encoder.sched.fluents.iter() {
                let t = Time::from(-2);
                let args = BBox::new(
                    params
                        .split_last()
                        .unwrap()
                        .1
                        .iter()
                        .map(|p| p.range)
                        .collect::<Vec<_>>(),
                );
                let mut grs = args.as_ref().points();
                while let Some(gr) = grs.next() {
                    res.push(Effect {
                        transition_start: t,
                        transition_end: t,
                        mutex_end: sched_encoder.store.new_ivar(-2, INT_CST_MAX, "_").into(),
                        state_var: StateVar {
                            fluent: sym.to_string(),
                            args: gr.iter().map(|&x| IntTerm::int_cst(x)).collect(),
                        },
                        operation: crate::EffectOp::Assign(IntTerm::ZERO),
                        prez: Lit::TRUE,
                        source: None,
                    });
                }
            }
            res
        };

        let empty_source_transitions_terms = {
            let mut res = BTreeSet::new();
            for e in sched_encoder.sched.effects.iter().chain(default_initial_effects.iter()) {
                if e.source.is_none() {
                    res.extend(TransitionRef::Eff(e).iter_terms().filter(|term| !term.is_cst()));
                }
            }
            for c in sched_encoder.causal_links.conditions.iter() {
                if c.source.is_none() {
                    res.extend(TransitionRef::Cond(c).iter_terms().filter(|term| !term.is_cst()));
                }
            }
            res.into_iter().collect()
        };

        let transitions = Transitions::from(
            sched_encoder,
            &empty_source_transitions_terms,
            &concrete_source_transitions_terms,
            &default_initial_effects,
        );

        Self {
            main: sched_encoder,
            transitions,
            empty_source_transitions_terms,
            concrete_source_transitions_terms,
            default_initial_effects,
            _default_initial_effects_starting_id,
            lprelax: None,
        }
    }
}
