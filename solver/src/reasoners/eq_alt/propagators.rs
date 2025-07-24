use hashbrown::HashMap;

use crate::{
    backtrack::{Backtrack, DecLvl, Trail},
    collections::{ref_store::RefVec, set::RefSet},
    core::{literals::Watches, Lit},
};

use super::{node::Node, relation::EqRelation};

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
pub(crate) struct ActivationEvent {
    /// the edge to enable
    pub edge: PropagatorId,
    /// The literals that enabled this edge to become active
    pub enabler: Enabler,
}

impl ActivationEvent {
    pub(crate) fn new(edge: PropagatorId, enabler: Enabler) -> Self {
        Self { edge, enabler }
    }
}

/// Represents an edge together with a particular propagation direction:
///  - forward (source to target)
///  - backward (target to source)
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct PropagatorId(u32);

impl From<PropagatorId> for usize {
    fn from(e: PropagatorId) -> Self {
        e.0 as usize
    }
}

impl From<usize> for PropagatorId {
    fn from(u: usize) -> Self {
        PropagatorId(u as u32)
    }
}

impl From<PropagatorId> for u32 {
    fn from(e: PropagatorId) -> Self {
        e.0
    }
}

impl From<u32> for PropagatorId {
    fn from(u: u32) -> Self {
        PropagatorId(u)
    }
}

/// One direction of a semi-reified eq or neq constraint.
///
/// The other direction will have flipped a and b, and different enabler.valid
#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct Propagator {
    pub a: Node,
    pub b: Node,
    pub relation: EqRelation,
    pub enabler: Enabler,
}

impl Propagator {
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
    MarkedActive(PropagatorId),
    MarkedValid(PropagatorId),
    EnablerAdded(PropagatorId),
}

#[derive(Clone, Default)]
pub struct PropagatorStore {
    propagators: RefVec<PropagatorId, Propagator>,
    propagator_indices: HashMap<(Node, Node), Vec<PropagatorId>>,
    marked_active: RefSet<PropagatorId>,
    watches: Watches<(Enabler, PropagatorId)>,
    trail: Trail<Event>,
}

impl PropagatorStore {
    pub fn add_propagator(&mut self, prop: Propagator) -> PropagatorId {
        self.trail.push(Event::PropagatorAdded);
        let id = self.propagators.len().into();
        self.propagators.push(prop.clone());
        id
    }

    pub fn watch_propagator(&mut self, id: PropagatorId, prop: Propagator) {
        let enabler = prop.enabler;
        self.watches.add_watch((enabler, id), enabler.active);
        self.watches.add_watch((enabler, id), enabler.valid);
        self.trail.push(Event::EnablerAdded(id));
    }

    pub fn get_propagator(&self, prop_id: PropagatorId) -> &Propagator {
        // self.propagators.get(&prop_id).unwrap()
        &self.propagators[prop_id]
    }

    pub fn mark_valid(&mut self, prop_id: PropagatorId) {
        let prop = self.get_propagator(prop_id).clone();
        if let Some(v) = self.propagator_indices.get_mut(&(prop.a, prop.b)) {
            if !v.contains(&prop_id) {
                self.trail.push(Event::MarkedValid(prop_id));
                v.push(prop_id);
            }
        } else {
            self.trail.push(Event::MarkedValid(prop_id));
            self.propagator_indices.insert((prop.a, prop.b), vec![prop_id]);
        }
    }

    /// Get valid propagators by source and target
    pub fn get_from_nodes(&self, source: Node, target: Node) -> Vec<PropagatorId> {
        self.propagator_indices
            .get(&(source, target))
            .cloned()
            .unwrap_or(vec![])
    }

    pub fn enabled_by(&self, literal: Lit) -> impl Iterator<Item = (Enabler, PropagatorId)> + '_ {
        self.watches.watches_on(literal)
    }

    pub fn marked_active(&self, prop_id: &PropagatorId) -> bool {
        self.marked_active.contains(*prop_id)
    }

    /// Marks prop as active, unmarking it as undecided in the process
    /// Returns true if change was made, else false
    pub fn mark_active(&mut self, prop_id: PropagatorId) {
        self.trail.push(Event::MarkedActive(prop_id));
        self.marked_active.insert(prop_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (PropagatorId, &Propagator)> + use<'_> {
        self.propagators.entries()
    }
}

impl Backtrack for PropagatorStore {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.trail.restore_last_with(|event| match event {
            Event::PropagatorAdded => {
                let last_prop_id: PropagatorId = (self.propagators.len() - 1).into();
                // let last_prop = self.propagators.get(&last_prop_id).unwrap().clone();
                // self.propagators.remove(&last_prop_id);
                self.marked_active.remove(last_prop_id);
            }
            Event::MarkedActive(prop_id) => {
                self.marked_active.remove(prop_id);
            }
            Event::MarkedValid(prop_id) => {
                let prop = &self.propagators[prop_id];
                let entry = self.propagator_indices.get_mut(&(prop.a, prop.b)).unwrap();
                entry.retain(|e| *e != prop_id);
                if entry.is_empty() {
                    self.propagator_indices.remove(&(prop.a, prop.b));
                }
            }
            Event::EnablerAdded(prop_id) => {
                let prop = &self.propagators[prop_id];
                self.watches.remove_watch((prop.enabler, prop_id), prop.enabler.active);
                self.watches.remove_watch((prop.enabler, prop_id), prop.enabler.valid);
            }
        });
    }
}
