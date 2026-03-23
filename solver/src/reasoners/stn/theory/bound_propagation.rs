use std::{cell::RefCell, cmp::Reverse};

use crate::{
    collections::{
        heap::IdxHeap,
        ref_store::{IterableRefMap, RefMap},
    },
    reasoners::stn::theory::{Identity, ModelUpdateCause},
};

use super::{
    INT_CST_MAX, IntCst, PropagatorId, SignedVar, StnTheory,
    state::{Domains, InvalidUpdate},
};

thread_local! {
    static STATE: RefCell<Dij> = RefCell::new(Dij::default())
}

/// Process all bound changes in `stn.pending_bound_changes` and propagate them by:
///  - updating the bounds of all active edges in the network
///  - activating/deactivating all edges that are entailed/diabled by the current bounds
///
/// As a result, the `pending_bound_change` queue is emptied
///
/// This corresponds roughly to the bound update part of
/// ref: Global Difference Constraint Propagation for Finite Domain Solvers, by Feydy, Schutt and Stuckey
/// modified to handle optional timepoints.
pub fn process_bound_changes(
    stn: &mut StnTheory,
    doms: &mut Domains,
    cycle_detection: impl Fn(SignedVar) -> bool,
) -> Result<(), InvalidUpdate> {
    // acquire our thread-local working memory to avoid repeated allocations
    STATE.with_borrow_mut(|dij| {
        dij.clear();

        for update in &stn.pending_bound_changes {
            dij.add_modified_bound(
                update.var,
                update.previous_ub,
                update.new_ub,
                update.is_from_bound_propagation,
            );
        }
        stn.pending_bound_changes.clear();

        dij.run(stn, doms, cycle_detection)
    })
}

#[derive(Default, Clone)]
struct Dij {
    modified_vars: Vec<SignedVar>,
    // a valid potential funciton for the graph
    // This corresponds to the value it had at the last propagation
    potential: RefMap<SignedVar, IntCst>,
    init: RefMap<SignedVar, IntCst>,
    heap: MinHeap,
}

impl Dij {
    pub fn clear(&mut self) {
        // clearing ref map is potentially very costly
        // only delete the modified elements
        for &v in &self.modified_vars {
            self.potential.remove(v);
            self.init.remove(v);
        }
        // check deactivated because potentially costly for these datastructures
        // debug_assert!(self.potential.is_empty() && self.init.is_empty());
        self.modified_vars.clear();
        self.heap.clear();
    }

    pub fn add_modified_bound(&mut self, v: SignedVar, previous_ub: IntCst, ub: IntCst, is_from_self: bool) {
        // println!("mod {v:?}  {previous_ub} -> {ub}");
        if previous_ub == ub {
            return; // strange but not our problem
        }
        if is_from_self {
            // we emitted this event ourselves which mean we can ignore it as well as all previous events
            if self.init.contains(v) {
                let pos = self.modified_vars.iter().position(|e| *e == v).unwrap();
                self.modified_vars.swap_remove(pos);
            }
            self.init.remove(v);
            self.potential.remove(v)
        } else if self.potential.contains(v) {
            // println!("  pot");
            // we assume updates are coming in order
            // so the potential should be the first previous ub (greatest value)
            debug_assert!(self.init.contains(v));
            debug_assert!(self.potential[v] > previous_ub);
            debug_assert!(self.init[v] > ub);
            self.init.insert(v, ub);
        } else {
            debug_assert!(!self.init.contains(v));
            self.potential.insert(v, previous_ub);
            self.modified_vars.push(v);
            self.init.insert(v, ub);
        }
    }

    #[inline(never)]
    pub fn run(
        &mut self,
        stn: &mut StnTheory,
        doms: &mut Domains,
        cyclic: impl Fn(SignedVar) -> bool,
    ) -> Result<(), InvalidUpdate> {
        #[cfg(debug_assertions)]
        let mut deactivations = hashbrown::HashSet::new();
        let origin_potential = INT_CST_MAX;
        for &v in &self.modified_vars {
            // println!("p {v:?}");
            if doms.present(v) == Some(false) {
                // println!("  absent");
                // a node might have be set as absent since its last domain update
                continue;
            }
            debug_assert!(stn.constraints.is_vertex(v));
            let ub = self.init[v];
            // println!("init {v:?}  <= {ub:?}");
            debug_assert_eq!(ub, doms.ub(v));
            let reduced_cost = origin_potential + ub - self.potential[v];
            self.heap.insert_init(v, reduced_cost);
        }

        while let Some((v, reduced_cost)) = self.heap.pop() {
            if doms.present(v) == Some(false) {
                // the variable disappeared, ignore
                continue;
            }
            debug_assert!(self.potential.contains(v), "potential should have been set already");
            let source_potential = self.potential[v];
            let new_source_ub = reduced_cost - origin_potential + self.potential[v];
            // println!("pop {v:?}  <= {new_source_ub:?}");
            if cyclic(v) {
                // we updated a node flagged as cycle detection.
                // this means that we have a negative cycle.
                // The cycle may contain only optional nodes. Since all edges in the cycle are active,
                // all nodes must have the same presence variable.
                // Force these to be absent.
                // Note that if the timepoints were not optional, this would trigger an invalid update (contradiction)
                // as expected.
                let pred = self.heap.pred.get(v).copied().unwrap();
                let prez = doms.presence(v);
                // insert a timestamp that will be used to viewing the graph as it was at the time of the propagation when explaining the inference
                stn.last_disabling_timestamp.insert(pred, stn.trail.next_event());
                let cause = stn.identity.inference(ModelUpdateCause::CyclicEdgePropagation(pred));
                doms.set(!prez, cause)?;
                debug_assert!(doms.present(v) == Some(false),);
                // the vertex is not present anymore, proceed to next
                continue;
            }

            if let Some(pred) = self.heap.pred.get(v).copied() {
                #[cfg(debug_assertions)]
                debug_assert!(
                    doms.ub(v) == self.potential[v]
                        || self.init.contains(v)
                        || deactivations.contains(&v.leq(doms.ub(v))),
                    "value was already updated by ourselves (and not by an edge deactivation)"
                );
                debug_assert!(stn.constraints[pred].target == v);
                debug_assert!(
                    doms.present(v) != Some(false),
                    "We should have ignored this case earlier"
                );
                // this is a propagation of the predecessor edge
                let cause = Identity::new(crate::reasoners::ReasonerId::Diff)
                    .inference(ModelUpdateCause::EdgePropagation(pred));
                let changed_something = doms.set_ub(v, new_source_ub, cause)?;
                debug_assert!(
                    changed_something || doms.ub(v) != self.potential[v],
                    "We should always change the bound except in the corner case where the literal was set as part of an edge deactivation"
                );
                stn.stats.bound_updates += 1;
                // there are two possibilities:
                //  - we successfully performed the update on the ub (in which case it must comply we the update of the predecessor)
                //  - the update resulted in a local inconsistency and the variable was inferred as absent
                // Note that a global inconsistency would have exited the function immediately
                debug_assert!({
                    let cause = &stn.constraints[pred];
                    debug_assert!(cause.target == v);
                    doms.ub(cause.target) == doms.ub(cause.source) + cause.weight || doms.present(v) == Some(false)
                });
            } else {
                debug_assert!(self.init.contains(v));
            }

            if doms.present(v) == Some(false) {
                // the update made it absent no need to further process it
                continue;
            }
            {
                // we have a new upper bound for v
                // check if this would deactivate any edge
                let x = v;
                // length of shortest path ORIGIN -> x
                let dist_o_x = new_source_ub;
                for out in stn.constraints.potential_out_edges(x) {
                    if !doms.entails(!out.presence) {
                        // we have a potential edge   x  -- w --> y
                        let y = out.target;
                        let w = out.weight;
                        // literal that would be a consequence of this edge activation
                        let consequence = y.leq(dist_o_x + w);

                        // length of shortest path  y -> ORIGIN
                        let dist_y_o = -doms.lb(y);

                        // length of cycle  through the edge and ORIGIN
                        let cycle_length = dist_o_x + w + dist_y_o;

                        if cycle_length < 0 {
                            // the edge cannot be present, deactivate it
                            debug_assert!(doms.entails(!consequence));
                            let cause = stn
                                .identity
                                .inference(ModelUpdateCause::TheoryPropagationBoundsDeactivation(out.id));

                            // disable the edge
                            let change = doms.set(!out.presence, cause)?;
                            if change {
                                stn.stats.num_bound_edge_deactivation += 1;
                                #[cfg(debug_assertions)]
                                deactivations.insert(!out.presence);
                                // println!("disabled: {:?}", !out.presence);
                            }
                        }
                    }
                }
            }
            for outgoing in &stn.active_propagators[v] {
                let target = outgoing.target;
                if doms.present(target) == Some(false) {
                    // useless update, skip
                    continue;
                }
                let current_ub = doms.ub(target);
                // println!("  out  -- {} -> target   [<= {current_ub}]", outgoing.weight);
                // get the potential of the target.
                // If unset, we initialize it to the current value of the domain
                // This update is required because the value may change as a result of propagation
                if !self.potential.contains(target) {
                    self.potential.insert(target, current_ub);
                    // keep track of each variable we touched to be able to more efficiently clear the data structures
                    self.modified_vars.push(target);
                }
                let target_potential = self.potential[target];
                debug_assert!(
                    source_potential + outgoing.weight - target_potential >= 0,
                    "Invalid potential function"
                );

                let new_target_ub = new_source_ub + outgoing.weight;
                if new_target_ub < current_ub {
                    let target_reduced_cost = reduced_cost + outgoing.weight + source_potential - target_potential;
                    self.heap.update(target, target_reduced_cost, outgoing.id)
                }
            }
        }
        Ok(())
    }
}

#[derive(Default, Clone)]
struct MinHeap {
    heap: IdxHeap<SignedVar, Reverse<IntCst>>,
    pred: IterableRefMap<SignedVar, PropagatorId>,
}

impl MinHeap {
    pub fn clear(&mut self) {
        self.heap.clear();
        self.pred.clear();
    }
    pub fn insert_init(&mut self, v: SignedVar, cost: IntCst) {
        self.heap.declare_element(v, Reverse(cost));
        self.heap.enqueue(v);
    }

    pub fn update(&mut self, v: SignedVar, cost: IntCst, pred: PropagatorId) {
        if !self.heap.is_declared(v) {
            self.heap.declare_element(v, Reverse(cost));
            debug_assert!(!self.pred.contains(v));
            self.pred.insert(v, pred);
            self.heap.enqueue(v);
        } else if self.heap.priority(v).0 > cost {
            // we have a smaller cost, update
            debug_assert!(
                self.heap.is_enqueued(v),
                "We reduced the cost of a node not in the queue"
            );
            self.pred.insert(v, pred);
            self.heap.change_priority(v, |p| *p = Reverse(cost))
        }
    }

    pub fn pop(&mut self) -> Option<(SignedVar, IntCst)> {
        let k = self.heap.pop();
        k.map(|v| (v, self.heap.priority(v).0))
    }
}
