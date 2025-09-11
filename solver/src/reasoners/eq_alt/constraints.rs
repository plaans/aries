use hashbrown::HashMap;
use std::fmt::Debug;

use crate::{
    backtrack::{Backtrack, DecLvl, Trail},
    collections::ref_store::RefVec,
    core::{literals::Watches, Lit},
    create_ref_type,
};

use super::{node::Node, relation::EqRelation};

// TODO: Identical to STN, maybe identify some other common logic and bump up to reasoner module

/// Enabling information for a propagator.
/// A propagator should be enabled iff both literals `active` and `valid` are true.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Enabler {
    /// A literal that is true (but not necessarily present) when the propagator must be active if present
    pub active: Lit,
    /// A literal that is true when the propagator is within its validity scope, i.e.,
    /// when is known to be sound to propagate a change from the source to the target.
    ///
    /// In the simplest case, we have `valid = presence(active)` since by construction
    /// `presence(active)` is true iff both variables of the constraint are present.
    ///
    /// `valid` might a more specific literal but always with the constraints that
    /// `presence(active) => valid`
    pub valid: Lit,
}

impl Enabler {
    pub fn new(active: Lit, valid: Lit) -> Enabler {
        Enabler { active, valid }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ActivationEvent {
    /// the edge to enable
    pub prop_id: ConstraintId,
}

impl ActivationEvent {
    pub(crate) fn new(prop_id: ConstraintId) -> Self {
        Self { prop_id }
    }
}

create_ref_type!(ConstraintId);

impl Debug for ConstraintId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Propagator {}", self.to_u32())
    }
}

/// One direction of a semi-reified eq or neq constraint.
///
/// The other direction will have flipped a and b, and different enabler.valid
#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub a: Node,
    pub b: Node,
    pub relation: EqRelation,
    pub enabler: Enabler,
}

impl Constraint {
    pub fn new(a: Node, b: Node, relation: EqRelation, active: Lit, valid: Lit) -> Self {
        Self {
            a,
            b,
            relation,
            enabler: Enabler::new(active, valid),
        }
    }

    pub fn new_pair(a: Node, b: Node, relation: EqRelation, active: Lit, ab_valid: Lit, ba_valid: Lit) -> (Self, Self) {
        (
            Self::new(a, b, relation, active, ab_valid),
            Self::new(b, a, relation, active, ba_valid),
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum Event {
    PropagatorAdded,
    WatchAdded((ConstraintId, Lit)),
}

/// Data structures to store propagators.
#[derive(Clone, Default)]
pub struct ConstraintStore {
    propagators: RefVec<ConstraintId, Constraint>,
    propagator_indices: HashMap<(Node, Node), Vec<ConstraintId>>,
    watches: Watches<(Enabler, ConstraintId)>,
    trail: Trail<Event>,
}

impl ConstraintStore {
    pub fn add_constraint(&mut self, prop: Constraint) -> ConstraintId {
        self.trail.push(Event::PropagatorAdded);
        let id = self.propagators.len().into();
        self.propagators.push(prop.clone());
        self.propagator_indices
            .entry((prop.a, prop.b))
            .and_modify(|e| e.push(id))
            .or_insert(vec![id]);
        id
    }

    pub fn add_watch(&mut self, id: ConstraintId, literal: Lit) {
        let enabler = self.propagators[id].enabler;
        self.watches.add_watch((enabler, id), literal);
        self.trail.push(Event::WatchAdded((id, literal)));
    }

    pub fn get_constraint(&self, prop_id: ConstraintId) -> &Constraint {
        // self.propagators.get(&prop_id).unwrap()
        &self.propagators[prop_id]
    }

    /// Get valid propagators by source and target
    pub fn get_from_nodes(&self, source: Node, target: Node) -> Vec<ConstraintId> {
        self.propagator_indices
            .get(&(source, target))
            .cloned()
            .unwrap_or(vec![])
    }

    pub fn enabled_by(&self, literal: Lit) -> impl Iterator<Item = (Enabler, ConstraintId)> + '_ {
        self.watches.watches_on(literal)
    }

    pub fn iter(&self) -> impl Iterator<Item = (ConstraintId, &Constraint)> + use<'_> {
        self.propagators.entries()
    }
}

impl Backtrack for ConstraintStore {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.trail.restore_last_with(|event| match event {
            Event::PropagatorAdded => {
                // let last_prop_id: PropagatorId = (self.propagators.len() - 1).into();
                // let last_prop = self.propagators.get(&last_prop_id).unwrap().clone();
                // self.propagators.remove(&last_prop_id);
                self.propagators.pop();
            }
            Event::WatchAdded((id, l)) => {
                let enabler = self.propagators[id].enabler;
                self.watches.remove_watch((enabler, id), l);
            }
        });
    }
}
