use std::cmp::Reverse;

use crate::{
    collections::{heap::IdxHeap, ref_store::RefMap},
    reasoners::stn::theory::{Identity, ModelUpdateCause},
};

use super::{
    state::{Domains, InvalidUpdate},
    IntCst, PropagatorId, SignedVar, StnTheory, INT_CST_MAX,
};

#[derive(Default, Clone)]
pub struct Dij {
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
        debug_assert!(self.potential.is_empty());
        debug_assert!(self.init.is_empty());
        self.modified_vars.clear();
        self.heap.clear();
    }

    pub fn add_modified_bound(&mut self, v: SignedVar, previous_ub: IntCst, ub: IntCst) {
        // println!("mod {v:?}  {previous_ub} -> {ub}");
        if previous_ub == ub {
            return; // strange but not our problem
        }
        if self.potential.contains(v) {
            // println!("  pot");
            // we assume updates are coming in order
            // so the potential should be the first previous ub (greatest value)
            debug_assert!(self.init.contains(v));
            debug_assert!(self.potential[v] > previous_ub);
            debug_assert!(self.init[v] > ub);
        } else {
            debug_assert!(!self.init.contains(v));
            self.potential.insert(v, previous_ub);
            self.modified_vars.push(v);
        }
        self.init.insert(v, ub);
    }

    #[inline(never)]
    pub fn run(
        &mut self,
        stn: &StnTheory,
        doms: &mut Domains,
        cyclic: impl Fn(SignedVar) -> bool,
    ) -> Result<(), InvalidUpdate> {
        let origin_potential = INT_CST_MAX; //*self.potential.values().max().unwrap();
        let mut count = 0;
        for &v in &self.modified_vars {
            // println!("p {v:?}");
            if doms.present(v) == Some(false) {
                // println!("  absent");
                // a node might have be set as absent since its last domain update
                continue;
            }
            if !stn.is_timepoint(v) {
                // println!("  no edges");
                continue;
            }
            let ub = self.init[v];
            // println!("init {v:?}  <= {ub:?}");
            if ub == doms.ub(v) {
                let reduced_cost = origin_potential + ub - self.potential[v];
                self.heap.insert_init(v, reduced_cost);
                count += 1;
            } else {
                // we got an UB event on `v` but
                // we were not notified of the last event setting its UB
                // this is only possible if we are the one that the UB
                // in this case we have already propagated it and there is nothing to be done.

                // check that this is indeed the case
                debug_assert!({
                    let last_change_cause = doms.get_event(doms.implying_event(v.leq(doms.ub(v))).unwrap()).cause;
                    let inference = last_change_cause.as_external_inference().unwrap();
                    inference.writer == stn.identity.writer_id
                        && matches!(
                            ModelUpdateCause::from(inference.payload),
                            ModelUpdateCause::EdgePropagation(_)
                        )
                });
            }
        }
        // println!("init count: {count}");
        if count == 0 {
            return Ok(());
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
                let pred = self.heap.pred.get(v).copied().unwrap();
                let prez = doms.presence(v);
                let cause = stn.identity.inference(ModelUpdateCause::CyclicEdgePropagation(pred));
                doms.set(!prez, cause)?;
            }

            if let Some(pred) = self.heap.pred.get(v).copied() {
                debug_assert!(
                    doms.ub(v) == self.potential[v] || self.init.contains(v),
                    "value was already updated by ourselves"
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
                debug_assert!(changed_something);
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
                            // if change {
                            //     self.stats.num_bound_edge_deactivation += 1;
                            // }
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
    pred: RefMap<SignedVar, PropagatorId>,
}

impl MinHeap {
    pub fn clear(&mut self) {
        for k in self.heap.keys() {
            self.pred.remove(k);
        }
        debug_assert!(self.pred.is_empty());
        self.heap.clear();
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
