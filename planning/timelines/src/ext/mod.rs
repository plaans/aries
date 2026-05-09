pub mod ground;
pub mod lprelax;
pub mod transitions;

use crate::constraints::HasValueAt;
use crate::encoder::CondId;
use crate::{Effect, EffectId, Task, TaskId};
use crate::{IntTerm, encoder::SchedEncoder};

use aries::core::views::Dom;
use aries::model::lang::ModelWrapper;
use aries::prelude::*;

use itertools::Itertools;
pub(crate) use transitions::*;

type Source = Option<TaskId>;

pub struct SchedEncoderExt<'a> {
    pub(crate) main: &'a mut SchedEncoder,

    pub(crate) transitions: Transitions,

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
    pub fn iter_effects(&self) -> (impl Iterator<Item = &Effect>, impl Iterator<Item = &Effect>) {
        (self.main.sched.effects.iter(), self.transitions.get_default_initial_effects().1.iter())
    }

    pub fn iter_conditions(&self) -> impl Iterator<Item = &HasValueAt> {
        self.main.causal_links.conditions.iter()
    }

    pub fn get_effect(&self, eff_id: EffectId) -> &Effect {
        let n = self.main.sched.effects.iter().try_len().unwrap();
        if eff_id < n {
            self.main.sched.effects.get(eff_id)
        } else {
            let (m, def_es) = self.transitions.get_default_initial_effects();
            debug_assert!(m == n);
            &def_es[eff_id - m]
        }
    }

    pub fn get_condition(&self, cond_id: CondId) -> &HasValueAt {
        self.main.causal_links.conditions.get(cond_id)
    }

    pub fn get_source(&self, source: &Source) -> Option<&Task> {
        source.map(|task_id| &self.main.sched.tasks[task_id])
    }

    pub fn get_source_terms(&self, source: &Source) -> &[IntTerm] {
        self.transitions.get_source_terms(source)
    }

    pub fn new(sched_encoder: &'a mut SchedEncoder) -> Self {
        let transitions = Transitions::new(sched_encoder);

        Self {
            main: sched_encoder,
            transitions,
            lprelax: None,
        }
    }
}
