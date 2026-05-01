pub mod ground;
pub mod lprelax;
pub mod transitions;

use std::collections::BTreeSet;

use crate::TaskId;
use crate::{IntTerm, encoder::SchedEncoder};

use aries::core::views::Dom;
use aries::model::lang::ModelWrapper;
use aries::prelude::*;

use idmap::DirectIdMap;
pub(crate) use transitions::*;

type Source = Option<TaskId>;

pub struct SchedEncoderExt {
    pub(crate) main: std::sync::Arc<SchedEncoder>,

    pub(crate) transitions: Transitions,

    empty_source_transitions_terms: Vec<IntTerm>,
    concrete_source_transitions_terms: DirectIdMap<TaskId, Vec<IntTerm>>,
    pub(crate) lprelax: Option<aries_lprelax::LpRelax>,
}

impl Dom for SchedEncoderExt {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.main.upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.main.presence(var)
    }
}
impl ModelWrapper for SchedEncoderExt {
    type Lbl = String;

    fn get_model(&self) -> &crate::Model {
        self.main.get_model()
    }

    fn get_model_mut(&mut self) -> &mut crate::Model {
        std::sync::Arc::get_mut(&mut self.main).unwrap().get_model_mut()
    }
}

impl SchedEncoderExt {
    pub fn get_source_terms(&self, source: &Source) -> &[IntTerm] {
        if let Some(task_id) = source {
            &self.concrete_source_transitions_terms[task_id]
        } else {
            &self.empty_source_transitions_terms
        }
    }

    pub fn new(sched_encoder: std::sync::Arc<SchedEncoder>) -> Self {
        let empty_source_transitions_terms = BTreeSet::from_iter(
            std::iter::chain(
                sched_encoder
                    .sched
                    .effects
                    .iter()
                    .enumerate()
                    .filter(|&(_, e)| e.source.is_none())
                    .flat_map(|(eff_id, _)| Transition::Eff(eff_id).get_terms(&sched_encoder).iter()),
                sched_encoder
                    .causal_links
                    .conditions
                    .iter()
                    .enumerate()
                    .filter(|&(_, c)| c.source.is_none())
                    .flat_map(|(cond_id, _)| Transition::Cond(cond_id).get_terms(&sched_encoder).iter()),
            )
            .filter(|term| !term.is_cst()),
        )
        .into_iter()
        .collect();

        let concrete_source_transitions_terms = {
            let mut res = DirectIdMap::<TaskId, BTreeSet<IntTerm>>::new();
            for e in sched_encoder.sched.effects.iter() {
                if let Some(task_id) = e.source {
                    if !res.contains_key(task_id) {
                        res.insert(task_id, BTreeSet::new());
                    }
                    res.get_mut(task_id).unwrap().extend(&e.state_var.args);
                }
            }
            for c in sched_encoder.causal_links.conditions.iter() {
                if let Some(task_id) = c.source {
                    if !res.contains_key(task_id) {
                        res.insert(task_id, BTreeSet::new());
                    }
                    res.get_mut(task_id).unwrap().extend(&c.state_var.args);
                }
            }
            DirectIdMap::from_iter(res.into_iter().map(|(task_id, set)| (task_id, Vec::from_iter(set))))
        };

        Self {
            main: sched_encoder.clone(),
            transitions: Transitions::from(
                &sched_encoder,
                &empty_source_transitions_terms,
                &concrete_source_transitions_terms,
            ),
            empty_source_transitions_terms,
            concrete_source_transitions_terms,
            lprelax: None,
        }
    }
}
