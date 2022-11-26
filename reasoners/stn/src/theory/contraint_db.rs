use crate::theory::edges::*;
use aries_collections::ref_store::RefVec;
use aries_core::literals::Watches;
use aries_core::{Lit, VarBound};
use std::collections::HashMap;
use std::ops::{Index, IndexMut};

/// Enabling information for a propagator.
/// A propagator should be enabled iff both literals `active` and `valid` are true.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    lookup: HashMap<Edge, u32>, // TODO: remove
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
        let constraint = &self.constraints[propagator];
        self.edges.fill_with(constraint.source, Vec::new);

        // FIXME: makes modification of an existing propagator set much harder
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

    /// Adds a new propagator.
    /// Returns the ID of the propagator set it was added to.
    /// If the addition contributes a new enabler for the set, then we return the enabler in the corresponding option.
    pub fn add_propagator(&mut self, prop: SPropagator) -> (PropagatorId, Option<Enabler>) {
        // first try to find a propagator set that is compatible
        for id in self.constraints.keys() {
            let existing = &mut self.constraints[id];
            if existing.source == prop.source && existing.target == prop.target {
                // on same
                if existing.weight == prop.weight {
                    // propagator with same weight exists, just add our propagators to if
                    if !existing.enablers.contains(&prop.enabler) {
                        existing.enablers.push(prop.enabler);
                        return (id, Some(prop.enabler));
                    } else {
                        return (id, None);
                    }
                } else if prop.weight.is_tighter_than(existing.weight) {
                    // the new propagator is strictly stronger
                    if existing.enablers.len() == 1 && existing.enablers[0] == prop.enabler {
                        // we have the same enablers, supersede the previous propagator
                        // FIXME: this updates the weights in the edges set, but without guarantee that it
                        //        is an edge of the propagator set
                        for e in self.edges[prop.source].iter_mut() {
                            if e.target == prop.target && e.weight == existing.weight {
                                e.weight = prop.weight;
                            }
                        }
                        existing.weight = prop.weight;

                        return (id, None);
                    }
                } else if existing.weight.is_tighter_than(prop.weight) {
                    // this propagator set is stronger than our own, ignore our own.
                    if existing.enablers.len() == 1 && existing.enablers[0] == prop.enabler {
                        return (id, None);
                    }
                }
            }
        }
        // could not unify it with an existing edge, just add it to the end
        let enabler = prop.enabler;
        let prop = Propagator {
            source: prop.source,
            target: prop.target,
            weight: prop.weight,
            enabler: None,
            enablers: vec![prop.enabler],
        };
        (self.constraints.push(prop), Some(enabler))
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
