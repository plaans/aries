use std::collections::HashMap;

use crate::backtrack::DecLvl;
use crate::core::state::Conflict;
use crate::core::state::Explainer;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::var_order::VarOrder;

#[derive(Clone, Debug)]
pub struct Activity {
    table: HashMap<VarRef, f32>,
    decay_factor: f32,
}

impl Activity {
    pub fn new(decay_factor: f32) -> Self {
        debug_assert!(0.0 <= decay_factor && decay_factor <= 1.0);
        Activity {
            table: HashMap::new(),
            decay_factor,
        }
    }

    /// Return the activity of the given variable.
    fn get(&self, var: VarRef) -> f32 {
        *self.table.get(&var).unwrap_or(&0.0)
    }

    /// Bump the activity of the given variable.
    fn bump(&mut self, var: VarRef) {
        let activity = self.get(var) + 1.0;
        self.table.insert(var, activity);
    }

    /// Decay the variable activity.
    fn decay(&mut self) {
        for activity in self.table.values_mut() {
            *activity *= self.decay_factor;
        }
    }
}

impl<Lbl: Label> VarOrder<Lbl> for Activity {
    fn conflict(
        &mut self,
        clause: &Conflict,
        _model: &Model<Lbl>,
        _explainer: &mut dyn Explainer,
        _backtrack_level: DecLvl,
    ) {
        self.decay();
        for literal in clause.literals() {
            self.bump(literal.variable());
        }
    }

    fn select(&self, model: &Model<Lbl>) -> Option<VarRef> {
        model
            .state
            .variables()
            .filter(|v| !model.state.is_bound(*v))
            .min_by(|v1, v2| self.get(*v1).partial_cmp(&self.get(*v2)).unwrap())
    }
}

impl Default for Activity {
    fn default() -> Self {
        Self::new(0.95)
    }
}
