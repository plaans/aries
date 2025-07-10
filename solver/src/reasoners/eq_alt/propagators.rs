use hashbrown::{HashMap, HashSet};

use crate::core::{literals::Watches, Lit};

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
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
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
#[derive(Clone, Hash, Debug)]
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

#[derive(Clone, Default)]
pub struct PropagatorStore {
    propagators: HashMap<PropagatorId, Propagator>,
    active_props: HashSet<PropagatorId>,
    watches: Watches<(Enabler, PropagatorId)>,
}

impl PropagatorStore {
    pub fn add_propagator(&mut self, prop: Propagator) -> PropagatorId {
        let id = self.propagators.len().into();
        let enabler = prop.enabler;
        self.propagators.insert(id, prop);
        self.watches.add_watch((enabler, id), enabler.active);
        self.watches.add_watch((enabler, id), enabler.valid);
        self.watches.add_watch((enabler, id), !enabler.valid);
        id
    }

    pub fn get_propagator(&self, prop_id: PropagatorId) -> &Propagator {
        self.propagators.get(&prop_id).unwrap()
    }

    pub fn enabled_by(&self, literal: Lit) -> impl Iterator<Item = (Enabler, PropagatorId)> + '_ {
        self.watches.watches_on(literal)
    }

    pub fn is_enabled(&self, prop_id: PropagatorId) -> bool {
        self.active_props.contains(&prop_id)
    }

    pub fn mark_active(&mut self, prop_id: PropagatorId) {
        debug_assert!(self.propagators.contains_key(&prop_id));
        self.active_props.insert(prop_id);
    }

    pub fn mark_inactive(&mut self, prop_id: PropagatorId) {
        debug_assert!(self.propagators.contains_key(&prop_id));
        assert!(self.active_props.remove(&prop_id));
    }

    #[allow(unused)]
    pub fn inactive_propagators(&self) -> impl Iterator<Item = (&PropagatorId, &Propagator)> {
        self.propagators.iter().filter(|(p, _)| !self.active_props.contains(*p))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PropagatorId, &Propagator)> + use<'_> {
        self.propagators.iter()
    }
}
