use std::sync::Arc;

use aries::model::lang::ModelWrapper;

use crate::*;

/// Structure that provide all the context for encoding the scheduling problem
/// into a CSP.
pub struct SchedEncoder {
    /// Scheduling problem that is being encoded
    pub sched: Arc<Sched>,
    /// solver's model that will be populated with all constraints and variables
    pub(crate) store: crate::Model,

    pub(crate) causal_links: CausalLinks,
}
impl SchedEncoder {
    pub fn new(sched: Arc<Sched>, store: crate::Model) -> Self {
        Self {
            sched,
            store,
            causal_links: CausalLinks {
                destinations: vec![],
                store: DirectIdMap::default(),
            },
        }
    }
}

impl Dom for SchedEncoder {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.store.upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.store.presence(var)
    }
}
impl ModelWrapper for SchedEncoder {
    type Lbl = String;

    fn get_model(&self) -> &crate::Model {
        &self.store
    }

    fn get_model_mut(&mut self) -> &mut crate::Model {
        &mut self.store
    }
}

pub type ConditionId = usize;
pub(crate) struct CausalLinks {
    pub destinations: Vec<HasValueAt>,
    pub store: DirectIdMap<ConditionId, Vec<(Lit, EffectId)>>,
}
