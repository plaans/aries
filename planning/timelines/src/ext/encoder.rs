mod transitions;

use aries::core::views::Dom;
use aries::model::lang::ModelWrapper;
use aries::prelude::*;
use idmap::intid::IntegerId;

use crate::constraints::HasValueAt;
use crate::encoder::CondId;
use crate::{Effect, EffectId, Task, TaskId};
use crate::{IntTerm, encoder::SchedEncoder};

use crate::ext::transition::*;
use transitions::Transitions;

pub(crate) type Source = Option<TaskId>;

pub(crate) struct SchedEncoderExt<'a> {
    pub main: &'a mut SchedEncoder,

    transitions: Transitions,

    pub lprelax: Option<aries_lprelax::LpRelax>,
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
    pub fn new(sched_encoder: &'a mut SchedEncoder) -> Self {
        let transitions = Transitions::new(sched_encoder);

        Self {
            main: sched_encoder,
            transitions,
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
        self.main.sched.effects.iter().enumerate()
    }
    pub fn iter_default_effects(&self) -> impl Iterator<Item = (EffectId, &Effect)> {
        self.transitions
            .default_initial_effects
            .iter()
            .enumerate()
            .map(|(i, e)| (i + self.transitions.first_default_initial_effect_id, e))
    }
    /*pub fn iter_effects(&self) -> impl Iterator<Item = (EffectId, &Effect)> {
        self.iter_nondefault_effects().chain(self.iter_default_effects())
    }*/
    pub fn iter_conditions(&self) -> impl Iterator<Item = (CondId, &HasValueAt)> {
        self.main.causal_links.conditions.iter().enumerate()
    }
    pub fn iter_transitions(&self) -> impl Iterator<Item = (TransitionId, Source)> {
        std::iter::chain(
            self.transitions
                .of_empty_source
                .iter()
                .map(|tr_id| (self.transitions.store[*tr_id], None)),
            self.transitions
                .of_concrete_source
                .iter()
                .flat_map(move |(task_id, tr_ids)| {
                    tr_ids
                        .iter()
                        .map(move |tr_id| (self.transitions.store[*tr_id], Some(task_id)))
                }),
        )
    }

    pub fn get_source_terms(&self, source: &Source) -> &[IntTerm] {
        source
            .map(|task_id| &self.transitions.concrete_source_transitions_terms[task_id])
            .unwrap_or(&self.transitions.empty_source_transitions_terms)
    }
    pub fn get_source(&self, source: &Source) -> Option<&Task> {
        source.map(|task_id| &self.main.sched.tasks[task_id])
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

    /// Returns the position of the transition's terms within the source's terms
    /// (i.e. the collection of all terms appearing in the source's transitions).
    /// None corresponds to a constant term.
    pub fn get_transition_terms_positions_in_source_terms(
        &self,
        transition_id: &TransitionId,
    ) -> Option<impl Iterator<Item = Option<usize>>> {
        self.transitions
            .transition_terms_indices_in_source
            .get(transition_id)
            .map(|v| v.iter().copied())
    }
    pub fn get_transition(&'a self, transition_id: TransitionId) -> TransitionRef<'a> {
        match transition_id {
            TransitionId::Cond(c_id) => TransitionRef::Cond(self.get_condition(c_id)),
            TransitionId::Eff(e_id) => TransitionRef::Eff(self.get_effect(e_id)),
            TransitionId::CondEff(c_id, e_id) => {
                TransitionRef::CondEff(self.get_condition(c_id), self.get_effect(e_id))
            }
        }
    }
    pub fn get_transition_of_condition(&'a self, condition_id: CondId) -> Option<TransitionId> {
        self.transitions
            .of_condition
            .get(condition_id)
            .map(|&i| self.transitions.store[i])
    }
    pub fn get_transition_of_effect(&'a self, effect_id: EffectId) -> Option<TransitionId> {
        self.transitions
            .of_effect
            .get(effect_id)
            .map(|&i| self.transitions.store[i])
    }
    pub fn get_transitions_of_source(&self, source: &Source) -> impl Iterator<Item = TransitionId> {
        match source {
            None => Some(self.transitions.of_empty_source.iter()),
            Some(task_id) => self.transitions.of_concrete_source.get(task_id).map(|v| v.iter()),
        }
        .unwrap_or([].iter())
        .map(|&i| self.transitions.store[i])
    }
}
