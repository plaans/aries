use aries::{core::views::Dom, prelude::*};
use smallvec::SmallVec;
use std::{
    fmt::{Debug, Formatter},
    ops::Index,
};

use crate::{
    StateVar, TaskId, Time,
    boxes::{BoxRef, BoxUniverse, Segment},
};

/// Represents an effect on a state variable.
/// The effect has a first transition phase `]transition_start, transition_end[` during which the
/// value of the state variable is unknown.
/// Exactly at time `transition_end`, the state variable `state_var` is update with `value`
/// (assignment or increase based on `operation`).
/// For assignment effects, this value will persist until another assignment effect starts its own transition.
#[derive(Clone, Eq, PartialEq)]
pub struct Effect {
    /// Time at which the transition to the new value will start
    pub transition_start: Time,
    /// Time at which the transition will end
    pub transition_end: Time,
    /// If specified, the assign effect is required to persist at least until all of these timepoints.
    pub mutex_end: Time,
    /// State variable affected by the effect
    pub state_var: StateVar,
    /// Operation carried out by the effect (value assignment, increase)
    pub operation: EffectOp,
    /// Presence literal indicating whether the effect is present
    pub prez: Lit,
    /// Specifies if this effect originates from a particular task.
    /// This is used to enforce the PDDL-mutex constraint that specifies
    /// that an aciton must not rely on a value that is immediately delete by *another* action.
    /// (mutex conditions).
    pub source: Option<TaskId>,
}
#[derive(Clone, Eq, PartialEq)]
pub enum EffectOp {
    Assign(IntCst),
}
impl EffectOp {
    pub const TRUE_ASSIGNMENT: EffectOp = EffectOp::Assign(1);
    pub const FALSE_ASSIGNMENT: EffectOp = EffectOp::Assign(0);
}
impl Debug for EffectOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectOp::Assign(val) => {
                write!(f, ":= {val:?}")
            }
        }
    }
}

impl Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}, {:?}] {:?} {:?}",
            self.transition_start, self.transition_end, self.state_var, self.operation
        )
    }
}

impl Effect {
    pub fn effective_start(&self) -> Time {
        self.transition_end
    }
    pub fn transition_start(&self) -> Time {
        self.transition_start
    }
    pub fn variable(&self) -> &StateVar {
        &self.state_var
    }
}

pub type EffectId = usize;

#[derive(Clone)]
pub struct Effects {
    effects: Vec<Effect>,
    /// Associates every effect to a `Box` in a universe.
    /// The box denotes a particular region of the state space that *may* be affected by the effect.
    /// The intuition if that
    ///
    /// Boxes are partitioned based on their state variables (one world per state variable).
    /// The box of each effect captures the space-time region it affects with dimesions:
    ///
    ///  - time: `[lb(transition_start), ub(mutex_end)]`
    ///  - for each parameter p:
    ///    - `[lb(p), ub(p)]`
    ///
    /// If the boxes of two effects to not overlap, they can be safely determined to never overlap (and thus do not require coherence enforcement constraints).
    affected_bb: BoxUniverse<String, usize>,
    /// Associates every effect to a `Box` in a universe.
    /// This box denotes a the set of values that the effect may support.
    /// The intuition if that
    ///
    /// Boxes are partitioned based on their state variables (one world per state variable).
    /// The box of each effect captures the space-time region it affects with dimesions:
    ///
    ///  - time: `[lb(transition_end), ub(mutex_end)]`
    ///  - for each parameter p:
    ///    - `[lb(p), ub(p)]`
    ///  - value: `[lb(value), ub(value)]`
    ///
    /// If the boxes of two effects to not overlap, they can be safely determined to never overlap (and thus do not require coherence enforcement constraints).
    achieved_bounding_boxes: BoxUniverse<String, usize>,
    /// Associates every effect to a `Box` in a universe.
    /// This box denotes a the set of values that the effect may support.
    /// The intuition if that
    ///
    /// Boxes are partitioned based on their state variables (one world per state variable).
    /// The box of each effect captures the space-time region it affects with dimesions:
    ///
    ///  - time: `[lb(transition_start), ub(transition_end))`
    ///  - for each parameter p:
    ///    - `[lb(p), ub(p)]`
    ///
    /// If the boxes of two effects to not overlap, they can be safely determined to never overlap (and thus do not require coherence enforcement constraints).
    transition_bounding_boxes: BoxUniverse<String, usize>,
}

type Segments = SmallVec<[Segment; 16]>;

impl Effects {
    pub fn new() -> Self {
        Self {
            effects: Default::default(),
            affected_bb: BoxUniverse::new(),
            achieved_bounding_boxes: BoxUniverse::new(),
            transition_bounding_boxes: BoxUniverse::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Effect> + '_ {
        self.effects.iter()
    }

    pub fn get(&self, eff_id: EffectId) -> &Effect {
        &self.effects[eff_id]
    }

    pub fn add_effect(&mut self, eff: Effect, dom: impl Dom) -> EffectId {
        // ID of the effect will be the index of the next free slot
        let eff_id = self.effects.len();

        let mut buff = Segments::new();

        // compute and store affected bounding_box
        buff.push(Segment::new(
            dom.lb(eff.transition_start.num),
            dom.ub(eff.mutex_end.num),
        )); // TODO: careful with denom
        for arg in &eff.state_var.args {
            let (lb, ub) = dom.bounds(*arg);
            buff.push(Segment::new(lb, ub));
        }
        self.affected_bb.add_box(&eff.state_var.fluent, &buff, eff_id);

        // compute and store the achievable bounding boxes
        buff.clear();
        buff.push(Segment::new(dom.lb(eff.transition_end.num), dom.ub(eff.mutex_end.num))); // TODO: careful with denom
        for arg in &eff.state_var.args {
            let (lb, ub) = dom.bounds(*arg);
            buff.push(Segment::new(lb, ub));
        }
        let value_segment = match &eff.operation {
            EffectOp::Assign(v) => Segment::new(*v, *v),
        };
        buff.push(value_segment);
        self.achieved_bounding_boxes
            .add_box(&eff.state_var.fluent, &buff, eff_id);

        // compute and store the achievable bounding boxes
        buff.clear();
        buff.push(Segment::new(
            dom.lb(eff.transition_start.num),
            dom.ub(eff.transition_end.num) - 1,
        )); // TODO: careful with denom
        for arg in &eff.state_var.args {
            let (lb, ub) = dom.bounds(*arg);
            buff.push(Segment::new(lb, ub));
        }
        self.transition_bounding_boxes
            .add_box(&eff.state_var.fluent, &buff, eff_id);

        self.effects.push(eff);
        eff_id
    }

    /// Returns a list of effects that may overlap on the state variable and overall activity period `[transition_start, mutex_end]`
    pub(crate) fn potentially_interacting_effects(&self) -> impl Iterator<Item = (EffectId, EffectId)> + '_ {
        self.affected_bb.overlapping_boxes().map(|(e1, e2)| (*e1, *e2))
    }

    /// Returns a list of potentially supporting effect for a condition, representing as a bounding box with
    /// the given fluents and the following segments:
    ///
    ///  - time: `[lb(condition_start), ub(condition_end)]`
    ///  - for each parameter p:
    ///    - `[lb(p), ub(p)]`
    ///  - value: `[lb(value), ub(value)]`
    pub(crate) fn potentially_supporting_effects<'a>(
        &'a self,
        fluent: &'a String,
        value_box: BoxRef<'a>,
    ) -> impl Iterator<Item = EffectId> + 'a {
        // compute the value bounding box

        self.achieved_bounding_boxes
            .find_overlapping_with(fluent, value_box)
            .copied()
    }

    /// Returns a list of effects whose transition period `[transition_start, transition_end)` may overlap a condition with the
    /// the given fluents and a given value_box (same bounding box as [`Self::potentially_supporting_effects`]).
    ///
    /// Note that the last segment of the box (representing the value) is ignored in the lookup.
    pub(crate) fn potentially_overlapping_transitions<'a>(
        &'a self,
        fluent: &'a String,
        value_box: BoxRef<'a>,
    ) -> impl Iterator<Item = EffectId> + 'a {
        // the same box but without the value
        let box_without_value = value_box.drop_tail(1);

        self.transition_bounding_boxes
            .find_overlapping_with(fluent, box_without_value)
            .copied()
    }
}

impl Default for Effects {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<EffectId> for Effects {
    type Output = Effect;

    fn index(&self, index: EffectId) -> &Self::Output {
        self.get(index)
    }
}
