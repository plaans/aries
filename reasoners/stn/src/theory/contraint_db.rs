use crate::theory::edges::*;
use crate::theory::{Timepoint, W};
use aries_collections::ref_store::RefVec;
use aries_core::literals::Watches;
use aries_core::{Lit, VarBound};
use std::collections::HashMap;
use std::ops::{Index, IndexMut};

/// Enabling information for a propagator.
/// A propagator should be enabled iff both literals `active` and `valid` are true.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Enabler {
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

/// Data structures that holds all active and inactive edges in the STN.
/// Note that some edges might be represented even though they were never inserted if they are the
/// negation of an inserted edge.
#[derive(Clone)]
pub(crate) struct ConstraintDb {
    /// All directional constraints.
    ///
    /// Each time a new edge is created four `DirConstraint` will be added
    /// - forward view of the canonical edge
    /// - backward view of the canonical edge
    /// - forward view of the negated edge
    /// - backward view of the negated edge
    constraints: RefVec<PropagatorId, Propagator>,
    /// Maps each canonical edge to its base ID.
    lookup: HashMap<Edge, u32>,
    /// Associates literals to the edges that should be activated when they become true
    watches: Watches<(Enabler, PropagatorId)>,
    edges: RefVec<VarBound, Vec<PropagatorTarget>>,
    /// Index of the next constraint that has not been returned yet by the `next_new_constraint` method.
    next_new_constraint: usize,
}

impl ConstraintDb {
    pub fn new() -> ConstraintDb {
        ConstraintDb {
            constraints: Default::default(),
            lookup: HashMap::new(),
            watches: Default::default(),
            edges: Default::default(),
            next_new_constraint: 0,
        }
    }

    pub fn num_propagators(&self) -> usize {
        self.constraints.len()
    }

    /// A function that acts as a one time iterator over constraints.
    /// It can be used to check if new constraints have been added since last time this method was called.
    pub fn next_new_constraint(&mut self) -> Option<&Propagator> {
        if self.next_new_constraint < self.constraints.len() {
            let out = &self.constraints[self.next_new_constraint.into()];
            self.next_new_constraint += 1;
            Some(out)
        } else {
            None
        }
    }

    /// Record the fact that:
    ///  - if `enabler` holds (both literals are true), then the propagator should be enabled
    ///  - if the `propagator` is inconsistent with the rest of the network, then the `enabler.active`
    ///    literal should be made false.
    pub fn add_propagator_enabler(&mut self, propagator: PropagatorId, enabler: Enabler) {
        // watch both the `active` and the `valid` literal.
        // when notified that one becomes true, we will need to check that the other is true before taking any action
        self.watches.add_watch((enabler, propagator), enabler.active);
        self.watches.add_watch((enabler, propagator), enabler.valid);
        let constraint = &mut self.constraints[propagator];
        constraint.enablers.push(enabler);
        self.edges.fill_with(constraint.source, Vec::new);

        self.edges[constraint.source].push(PropagatorTarget {
            target: constraint.target,
            weight: constraint.weight,
            presence: enabler.active,
        });
    }

    pub fn potential_out_edges(&self, source: VarBound) -> &[PropagatorTarget] {
        if self.edges.contains(source) {
            &self.edges[source]
        } else {
            &[]
        }
    }

    fn find_existing(&self, edge: &Edge) -> Option<EdgeId> {
        if edge.is_canonical() {
            self.lookup.get(edge).map(|&id| EdgeId::new(id, false))
        } else {
            self.lookup.get(&edge.negated()).map(|&id| EdgeId::new(id, true))
        }
    }

    /// Adds a new edge and return a pair (created, edge_id) where:
    ///  - created is false if NO new edge was inserted (it was merge with an identical edge already in the DB)
    ///  - edge_id is the id of the edge
    pub fn push_edge(&mut self, source: Timepoint, target: Timepoint, weight: W) -> (bool, EdgeId) {
        let edge = Edge::new(source, target, weight);
        match self.find_existing(&edge) {
            Some(id) => {
                // edge already exists in the DB, return its id and say it wasn't created
                debug_assert_eq!(self[PropagatorId::forward(id)].as_edge(), edge);
                debug_assert_eq!(self[PropagatorId::backward(id)].as_edge(), edge);
                (false, id)
            }
            None => {
                // edge does not exist, record the corresponding pair and return the new id.
                let pair = ConstraintPair::new_inactives(edge);
                let base = pair.base_forward.as_edge();
                let id1 = self.constraints.push(pair.base_forward);
                let _ = self.constraints.push(pair.base_backward);
                let id2 = self.constraints.push(pair.negated_forward);
                let _ = self.constraints.push(pair.negated_backward);
                self.lookup.insert(base, id1.edge().base_id());
                debug_assert_eq!(id1.edge().base_id(), id2.edge().base_id());
                let edge_id = if edge.is_negated() { id2 } else { id1 };
                (true, edge_id.edge())
            }
        }
    }

    pub fn enabled_by(&self, literal: Lit) -> impl Iterator<Item = (Enabler, PropagatorId)> + '_ {
        self.watches.watches_on(literal)
    }

    /// Removes the last created ConstraintPair in the DB. Note that this will remove the last edge that was
    /// pushed and THAT WAS NOT UNIFIED with an existing edge (i.e. edge_push returned : (true, _)).
    pub fn pop_last(&mut self) {
        debug_assert_eq!(self.constraints.len() % 4, 0);
        // remove the four edges (forward and backward) for both the base and negated edge
        self.constraints.pop();
        self.constraints.pop();
        self.constraints.pop();
        if let Some(c) = self.constraints.pop() {
            debug_assert!(c.as_edge().is_canonical());
            self.lookup.remove(&c.as_edge());
        }
    }

    pub fn has_edge(&self, id: EdgeId) -> bool {
        id.base_id() <= self.constraints.len() as u32
    }
}
impl Index<PropagatorId> for ConstraintDb {
    type Output = Propagator;

    fn index(&self, index: PropagatorId) -> &Self::Output {
        &self.constraints[index]
    }
}
impl IndexMut<PropagatorId> for ConstraintDb {
    fn index_mut(&mut self, index: PropagatorId) -> &mut Self::Output {
        &mut self.constraints[index]
    }
}

/// A pair of constraints (a, b) where edge(a) = !edge(b)
struct ConstraintPair {
    /// constraint where the edge is in its canonical form
    base_forward: Propagator,
    base_backward: Propagator,
    /// constraint corresponding to the negation of base
    negated_forward: Propagator,
    negated_backward: Propagator,
}

impl ConstraintPair {
    pub fn new_inactives(edge: Edge) -> ConstraintPair {
        let edge = if edge.is_canonical() { edge } else { edge.negated() };
        ConstraintPair {
            base_forward: Propagator::forward(edge),
            base_backward: Propagator::backward(edge),
            negated_forward: Propagator::forward(edge.negated()),
            negated_backward: Propagator::backward(edge.negated()),
        }
    }
}