use std::{collections::BTreeMap, sync::Arc};

use aries::model::lang::ModelWrapper;

use crate::*;

/// Structure that provide all the context for encoding the scheduling problem
/// into a CSP.
pub struct SchedEncoder {
    /// Scheduling problem that is being encoded
    pub sched: Arc<Sched>,
    /// solver's model that will be populated with all constraints and variables
    pub(crate) store: crate::Model,
    /// Accumulator for all causal links in the encoding.
    pub causal_links: CausalLinks,
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

/// Identifies a condition in the timelines.
///
/// In general, conditions are to be understood more broadly than in classical planning and may refer
/// to anything that requires a value at a given point in time. Essentially an [`HasValueAt`] constraint leads to
/// a `Cond`. This typically includes a normal precondition as in PDDL, but also the result of storing an expression
/// on a state variable into an intermediate variable.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cond {
    /// Source of the condition, typically a Task in which the condition appears or `None` if it is introduced
    /// in the global scope (problem).
    pub source: Option<TaskId>,
    /// Indicates that this is the `n^th` condition recorded for this task
    pub cond_id: u32,
}

/// A causal link identifies a dependency relationship between an effect and a condition.
///
/// If the link is `active` then the effect participates in defining the value of the condition.
/// Note that while exactly one assignment effect will participate (defining the base value) there might be
/// (in addition) more than one increase effect (defining the increments from the base value).
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CausalLink {
    /// ID of the effect that may participate
    pub eff: EffectId,
    /// Condition that may be affected by the effect
    pub cond: Cond,
    /// A literal that is true if the effect participates in defining the value.
    /// Note that the literal is typically optional and only required to be present if the condition is present as well.
    /// It must be false if the effect is absent however.
    pub active: Lit,
}

/// Accumulates the set of all [`CausalLink`]s in an encoding problem.
///
/// These are accumulated when encoding [`HasValueAt`] constraints.
#[derive(Default)]
pub struct CausalLinks {
    /// Debug util: this is used to make sure all causal links have been added before any read.
    /// If a new causal link is added *after* a read access, the corresponding method will panic.
    /// This is a workaround for the current lack of phased encoding that would statically ensure this.
    has_been_read: std::cell::Cell<bool>,
    condition_counts: BTreeMap<Option<TaskId>, u32>,
    links: Vec<CausalLink>,
}

impl CausalLinks {
    /// Records a new condition with the associated causal links.
    pub fn add_new_condition_participants(&mut self, source: Option<TaskId>, supports: Vec<(EffectId, Lit)>) {
        debug_assert!(!self.has_been_read.get());
        let count_for_task = self.condition_counts.entry(source).or_default();
        let cond_id = *count_for_task;
        *count_for_task += 1;
        let cond = Cond { source, cond_id };
        for (eff, enforced) in supports {
            self.links.push(CausalLink {
                eff,
                cond,
                active: enforced,
            });
        }
    }

    /// Returns all causal links *accumulated so far*.
    ///
    /// It is the responsibility of the caller to ensure that this method is called *after* all causal links have been added
    /// (typically by placing the corresponding constraint last in the queue).
    pub fn get_links(&self) -> impl Iterator<Item = &CausalLink> {
        self.has_been_read.set(true);
        self.links.iter()
    }
}
