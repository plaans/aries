#![allow(unused)] // TODO: remove once stabilized

pub mod disjunctive;
pub mod linear;
pub mod max;
pub mod mul;
pub mod mul_lit;

mod propagator;
pub use propagator::*;

use crate::backtrack::{Backtrack, DecLvl, ObsTrailCursor};
use crate::collections::ref_store::{RefMap, RefVec};
use crate::collections::*;
use crate::core::state::{Domains, DomainsSnapshot, Event, Explanation, InferenceCause};
use crate::core::{Lit, SignedVar, VarRef};
use crate::model::lang::linear::NFLinearLeq;
use crate::model::lang::mul::{EqMul, NFEqVarMulLit};
use crate::reasoners::cp::linear::{LinearSumLeq, SumElem};
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use mul_lit::VarEqVarMulLit;
use set::IterableRefSet;

#[derive(Clone, Default)]
pub struct Watches {
    propagations: RefMap<SignedVar, Vec<PropagatorId>>,
}

impl Watches {
    /// Request a trigger of `propagator_id` on every bound change (lower or upper bound) of the `
    pub fn add_watch(&mut self, watched: VarRef, propagator_id: PropagatorId) {
        self.add_ub_watch(watched, propagator_id);
        self.add_lb_watch(watched, propagator_id);
    }

    /// Request a trigger of `propagator_id` on every upper bound change of the `watched` signed variable.
    /// If `watched` is given as a VarRef, notification will occur on the change of its upper bound.
    pub fn add_ub_watch(&mut self, watched: impl Into<SignedVar>, propagator_id: PropagatorId) {
        let watched = watched.into();
        self.propagations
            .get_mut_or_insert(watched, || Vec::with_capacity(4))
            .push(propagator_id)
    }

    /// Request a trigger of `propagator_id` on every lower bound change of the `watched` signed variable.
    /// If `watched` is given as a VarRef, notification will occur on the change of its lower bound.
    pub fn add_lb_watch(&mut self, watched: impl Into<SignedVar>, propagator_id: PropagatorId) {
        let watched = watched.into();
        self.add_ub_watch(-watched, propagator_id)
    }

    /// Request a trigger of `propagator_id` when the `watched` literal becomes true.
    /// Note: due to the implementation of watches not being very fine-grained, the current implementation may trigger propagation on every
    /// upper bound change of the underlying literal (subject to change in the future).
    pub fn add_lit_watch(&mut self, watched: impl Into<Lit>, propagator_id: PropagatorId) {
        let watched = watched.into();
        self.add_ub_watch(watched.svar(), propagator_id);
    }

    /// Returns all propagators
    fn get_ub_watches(&self, var: impl Into<SignedVar>) -> &[PropagatorId] {
        let var = var.into();
        self.propagations.get(var).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[derive(Clone)]
pub struct Stats {
    pub num_propagations: u64,
}

#[allow(clippy::derivable_impls)]
impl Default for Stats {
    fn default() -> Self {
        Self { num_propagations: 0 }
    }
}

#[derive(Clone)]
pub struct Cp {
    id: ReasonerId,
    constraints: RefVec<PropagatorId, DynPropagator>,
    model_events: ObsTrailCursor<Event>,
    watches: Watches,
    saved: DecLvl,
    /// Propagators that have never been propagated to this point
    pending_propagators: Vec<PropagatorId>,
    /// Datastructure used in `propagate` to keep track of which propagators should be triggered.
    /// Not stateful. Present here only to avoid reallocations
    pending_propagations: IterableRefSet<PropagatorId>,
    pub stats: Stats,
}

impl Cp {
    pub fn new(id: ReasonerId) -> Cp {
        Cp {
            id,
            constraints: Default::default(),
            model_events: ObsTrailCursor::new(),
            watches: Default::default(),
            saved: DecLvl::ROOT,
            pending_propagators: Default::default(),
            pending_propagations: Default::default(),
            stats: Default::default(),
        }
    }

    pub fn add_linear_constraint(&mut self, leq: &NFLinearLeq, doms: &Domains) {
        self.add_half_reif_linear_constraint(leq, Lit::TRUE, doms)
    }

    /// Adds a linear constraint that is only active when `active` is true.
    pub fn add_half_reif_linear_constraint(&mut self, leq: &NFLinearLeq, active: Lit, doms: &Domains) {
        let valid = doms.presence(active);
        debug_assert!(leq.sum.iter().all(|e| doms.implies(valid, doms.presence(e.var))));
        let elements = leq.sum.iter().map(|e| SumElem::new(e.factor, e.var)).collect();
        let propagator = LinearSumLeq {
            elements,
            ub: leq.upper_bound,
            active,
            valid,
        };
        self.add_propagator(propagator);
    }

    pub fn add_half_reified_mul_constraint(&mut self, mul: &EqMul, active: Lit, doms: &Domains) {
        // TODO: this is correct but may miss opportunities for eager propagation of optional variables
        let valid = doms.presence(active);
        debug_assert!(
            [mul.lhs, mul.rhs1, mul.rhs2]
                .iter()
                .all(|e| doms.implies(valid, doms.presence(*e)))
        );
        let propagator = mul::Mul {
            prod: mul.lhs,
            fact1: mul.rhs1,
            fact2: mul.rhs2,
            active,
            valid,
        };
        self.add_propagator(propagator);
    }

    pub fn add_eq_var_mul_lit_constraint(&mut self, mul: &NFEqVarMulLit) {
        let propagator = VarEqVarMulLit {
            reified: mul.lhs,
            original: mul.rhs,
            lit: mul.lit,
        };
        self.add_propagator(propagator);
    }

    pub fn add_propagator(&mut self, propagator: impl Into<DynPropagator>) {
        // TODO: handle validity scopes
        let propagator = propagator.into();
        let propagator_id = self.constraints.next_key();
        propagator.constraint.setup(propagator_id, &mut self.watches);
        self.constraints.set_next(propagator_id, propagator);
        // mark the constraint as pending for propagation
        self.pending_propagators.push(propagator_id);
    }
}

impl Theory for Cp {
    fn identity(&self) -> ReasonerId {
        self.id
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        // clean up
        self.pending_propagations.clear();

        // schedule propagators that have never been triggered
        for propagator in self.pending_propagators.drain(..) {
            debug_assert_eq!(
                domains.current_decision_level(),
                DecLvl::ROOT,
                "First propagation should occur at root."
            );
            self.pending_propagations.insert(propagator)
        }

        // add any propagator that watches a bound updated since last propagation
        while let Some(event) = self.model_events.pop(domains.trail()) {
            let watchers = self.watches.get_ub_watches(event.affected_bound);
            for &watcher in watchers {
                // note: this could be improved as we may be rescheduling the propagator that triggered the event
                self.pending_propagations.insert(watcher);
            }
        }

        for propagator in self.pending_propagations.iter() {
            let constraint = self.constraints[propagator].constraint.as_ref();
            let cause = self.id.cause(propagator);
            self.stats.num_propagations += 1;
            constraint.propagate(domains, cause)?;
        }

        Ok(())
    }

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        state: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        let constraint_id = PropagatorId::from(context.payload);
        let constraint = self.constraints[constraint_id].constraint.as_ref();
        constraint.explain(literal, state, out_explanation);
    }

    fn print_stats(&self) {
        println!("# constraints: {}", self.constraints.len());
        println!("# propagations: {}", self.stats.num_propagations);
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

impl Backtrack for Cp {
    fn save_state(&mut self) -> DecLvl {
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.saved -= 1;
    }
}
