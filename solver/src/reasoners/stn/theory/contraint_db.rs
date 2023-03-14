use crate::backtrack::{Backtrack, DecLvl, Trail};
use crate::collections::ref_store::RefVec;
use crate::core::literals::Watches;
use crate::core::{Lit, SignedVar};
use crate::reasoners::stn::theory::edges::*;
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

#[derive(Clone, Copy)]
enum Event {
    PropagatorGroupAdded,
    /// An intermittent propagator was added for the given source
    EnablerAdded(PropagatorId, Enabler),
}

/// Data structures that holds all active and inactive edges in the STN.
/// Note that some edges might be represented even though they were never inserted if they are the
/// negation of an inserted edge.
#[derive(Clone)]
pub(crate) struct ConstraintDb {
    /// All propagators. Propagators that only differ by their enabler are grouped together.
    ///
    /// Each time a new edge is created four `Propagator`s will be added
    /// - forward view of the canonical edge
    /// - backward view of the canonical edge
    /// - forward view of the negated edge
    /// - backward view of the negated edge
    propagators: RefVec<PropagatorId, PropagatorGroup>,
    /// Associates each pair of nodes in the STN to their edges.
    propagator_indices: HashMap<(SignedVar, SignedVar), Vec<PropagatorId>>,
    /// Associates literals to the edges that should be activated when they become true
    watches: Watches<(Enabler, PropagatorId)>,
    /// Propagators whose activity depends on the current state.
    /// They are encoded in a compact form to speed-up processing: the vector is indexed by the source
    /// and associated each with the (weight, target, presence) of one of its propagators.
    intermittent_propagators: RefVec<SignedVar, Vec<PropagatorTarget>>,
    /// Index of the next constraint that has not been returned yet by the `next_new_constraint` method.
    next_new_constraint: usize,
    /// Backtrackable set of events, to allow resetting the network to a previous state.
    trail: Trail<Event>,
}

impl ConstraintDb {
    pub fn new() -> ConstraintDb {
        ConstraintDb {
            propagators: Default::default(),
            propagator_indices: Default::default(),
            watches: Default::default(),
            intermittent_propagators: Default::default(),
            next_new_constraint: 0,
            trail: Default::default(),
        }
    }

    pub fn num_propagator_groups(&self) -> usize {
        self.propagators.len()
    }

    /// A function that acts as a one time iterator over constraints.
    /// It can be used to check if new constraints have been added since last time this method was called.
    pub fn next_new_constraint(&mut self) -> Option<&PropagatorGroup> {
        if self.next_new_constraint < self.propagators.len() {
            let out = &self.propagators[self.next_new_constraint.into()];
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
        let constraint = &self.propagators[propagator];
        self.intermittent_propagators.fill_with(constraint.source, Vec::new);

        self.intermittent_propagators[constraint.source].push(PropagatorTarget {
            target: constraint.target,
            weight: constraint.weight,
            presence: enabler.active,
        });
        self.trail.push(Event::EnablerAdded(propagator, enabler));
    }

    pub fn potential_out_edges(&self, source: SignedVar) -> &[PropagatorTarget] {
        if self.intermittent_propagators.contains(source) {
            &self.intermittent_propagators[source]
        } else {
            &[]
        }
    }

    /// Adds a new propagator.
    /// Returns the ID of the propagator set it was added to and a description for how the integration was made.
    pub fn add_propagator(&mut self, prop: Propagator) -> (PropagatorId, PropagatorIntegration) {
        if self.trail.current_decision_level() == DecLvl::ROOT {
            // At the root level, try to optimize the organization of the propagators
            // It can not (easily) be done beyond the root level because it makes undoing it harder.

            // first try to find a propagator set that is compatible
            self.propagator_indices.entry((prop.source, prop.target)).or_default();
            for &id in &self.propagator_indices[&(prop.source, prop.target)] {
                let existing = &mut self.propagators[id];
                if existing.source == prop.source && existing.target == prop.target {
                    // on same
                    if existing.weight == prop.weight {
                        // propagator with same weight exists, just add our propagators to if
                        if !existing.enablers.contains(&prop.enabler) {
                            existing.enablers.push(prop.enabler);
                            return (id, PropagatorIntegration::Merged(prop.enabler));
                        } else {
                            return (id, PropagatorIntegration::Noop);
                        }
                    } else if prop.weight.is_tighter_than(existing.weight) {
                        // the new propagator is strictly stronger
                        if existing.enablers.len() == 1 && existing.enablers[0] == prop.enabler {
                            // We have the same enablers, supersede the previous propagator.

                            // If there is an intermittent propagator, update it.
                            // There can be only one since the group has a single enabler.
                            if self.intermittent_propagators.contains(prop.source) {
                                for e in self.intermittent_propagators[prop.source].iter_mut() {
                                    if e.target == prop.target
                                        && e.weight == existing.weight
                                        && e.presence == prop.enabler.active
                                    {
                                        e.weight = prop.weight;
                                        break;
                                    }
                                }
                            }
                            existing.weight = prop.weight;

                            return (id, PropagatorIntegration::Tightened(prop.enabler));
                        }
                    } else if existing.weight.is_tighter_than(prop.weight) {
                        // this existing propagator is stronger than our own, ignore our own.
                        if existing.enablers.len() == 1 && existing.enablers[0] == prop.enabler {
                            return (id, PropagatorIntegration::Noop);
                        }
                    }
                }
            }
        }
        // could not unify it with an existing edge, just add it to the end
        let source = prop.source;
        let target = prop.target;
        let enabler = prop.enabler;
        let prop = PropagatorGroup {
            source: prop.source,
            target: prop.target,
            weight: prop.weight,
            enabler: None,
            enablers: vec![prop.enabler],
        };
        let id = self.propagators.push(prop);
        self.propagator_indices.entry((source, target)).or_default().push(id);
        self.trail.push(Event::PropagatorGroupAdded);

        (id, PropagatorIntegration::Created(enabler))
    }

    pub fn enabled_by(&self, literal: Lit) -> impl Iterator<Item = (Enabler, PropagatorId)> + '_ {
        self.watches.watches_on(literal)
    }
}
impl Index<PropagatorId> for ConstraintDb {
    type Output = PropagatorGroup;

    fn index(&self, index: PropagatorId) -> &Self::Output {
        &self.propagators[index]
    }
}
impl IndexMut<PropagatorId> for ConstraintDb {
    fn index_mut(&mut self, index: PropagatorId) -> &mut Self::Output {
        &mut self.propagators[index]
    }
}

/// Description of how a propagator was added to a propagator set.
pub(crate) enum PropagatorIntegration {
    /// The propagator was added to a newly created propagator group, with the given enabler
    Created(Enabler),
    /// The propagator was added to an existing propagator group in which
    /// the given enabler was added
    Merged(Enabler),
    /// The propagator was redundant with an existing one and was ignored.
    Noop,
    /// The propagator superseded a previous group that was tightened.
    Tightened(Enabler),
}

impl Backtrack for ConstraintDb {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        self.trail.restore_last_with(|e| match e {
            Event::PropagatorGroupAdded => {
                let prop = self.propagators.pop().unwrap();
                self.propagator_indices
                    .get_mut(&(prop.source, prop.target))
                    .unwrap()
                    .pop();
            }
            Event::EnablerAdded(propagator, enabler) => {
                // undo the `add_propagator_enabler` method
                self.watches.remove_watch((enabler, propagator), enabler.active);
                self.watches.remove_watch((enabler, propagator), enabler.valid);
                let constraint = &self.propagators[propagator];
                self.intermittent_propagators[constraint.source].pop();
            }
        })
    }
}
