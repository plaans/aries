#![allow(unused)] // TODO: remove once stabilized

pub mod linear;
pub mod max;
pub mod mul;

use crate::backtrack::{Backtrack, DecLvl, ObsTrailCursor};
use crate::collections::ref_store::RefVec;
use crate::collections::*;
use crate::core::state::{Cause, Domains, Event, Explainer, Explanation, InferenceCause, InvalidUpdate};
use crate::core::{IntCst, Lit, SignedVar, VarRef, INT_CST_MAX, INT_CST_MIN};
use crate::create_ref_type;
use crate::model::extensions::AssignmentExt;
use crate::model::lang::linear::NFLinearLeq;
use crate::model::lang::mul::NFEqVarMulLit;
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use anyhow::Context;
use mul::VarEqVarMulLit;

use crate::reasoners::cp::linear::{LinearSumLeq, SumElem};
use crate::reasoners::cp::max::AtLeastOneGeq;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

// ========== Constraint ===========

create_ref_type!(PropagatorId);

trait Propagator: Send {
    fn setup(&self, id: PropagatorId, context: &mut Watches);
    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction>;
    fn propagate_event(&self, _event: &Event, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        self.propagate(domains, cause)
    }

    fn explain(&self, literal: Lit, state: &Domains, out_explanation: &mut Explanation);

    fn clone_box(&self) -> Box<dyn Propagator>;
}

impl<T: Propagator> Explainer for T {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &Domains, explanation: &mut Explanation) {
        Propagator::explain(self, literal, model, explanation)
    }
}

pub struct DynPropagator {
    constraint: Box<dyn Propagator>,
}

impl Clone for DynPropagator {
    fn clone(&self) -> Self {
        DynPropagator {
            constraint: self.constraint.clone_box(),
        }
    }
}

impl<T: Propagator + 'static> From<T> for DynPropagator {
    fn from(propagator: T) -> Self {
        DynPropagator {
            constraint: Box::new(propagator),
        }
    }
}

// ========= CP =============

#[derive(Clone, Default)]
pub struct Watches {
    propagations: HashMap<VarRef, Vec<PropagatorId>>,
    empty: [PropagatorId; 0],
}

impl Watches {
    fn add_watch(&mut self, watched: VarRef, propagator_id: PropagatorId) {
        self.propagations
            .entry(watched)
            .or_insert_with(|| Vec::with_capacity(4))
            .push(propagator_id)
    }

    fn get(&self, var_bound: VarRef) -> &[PropagatorId] {
        self.propagations
            .get(&var_bound)
            .map(|v| v.as_slice())
            .unwrap_or(&self.empty)
    }
}

#[derive(Clone)]
pub struct Cp {
    id: ReasonerId,
    constraints: RefVec<PropagatorId, DynPropagator>,
    model_events: ObsTrailCursor<Event>,
    watches: Watches,
    saved: DecLvl,
}

impl Cp {
    pub fn new(id: ReasonerId) -> Cp {
        Cp {
            id,
            constraints: Default::default(),
            model_events: ObsTrailCursor::new(),
            watches: Default::default(),
            saved: DecLvl::ROOT,
        }
    }

    pub fn add_linear_constraint(&mut self, leq: &NFLinearLeq) {
        self.add_opt_linear_constraint(leq, Lit::TRUE)
    }

    /// Adds a linear constraint that is only active when `active` is true.
    pub fn add_opt_linear_constraint(&mut self, leq: &NFLinearLeq, active: Lit) {
        let elements = leq.sum.iter().map(|e| SumElem::new(e.factor, e.var)).collect();
        let propagator = LinearSumLeq {
            elements,
            ub: leq.upper_bound,
            active,
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
    }
}

impl Theory for Cp {
    fn identity(&self) -> ReasonerId {
        self.id
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        // list of all propagators to trigger
        let mut to_propagate = HashSet::with_capacity(64);

        // in first propagation, mark everything for propagation
        // Note: this is might actually be triggered multiple times when going back to the root
        if self.saved == DecLvl::ROOT {
            for (id, p) in self.constraints.entries() {
                to_propagate.insert(id);
            }
        }

        // add any propagator that watched a changed variable since last propagation
        while let Some(event) = self.model_events.pop(domains.trail()).copied() {
            let watchers = self.watches.get(event.affected_bound.variable());
            for &watcher in watchers {
                to_propagate.insert(watcher);
            }
        }

        for propagator in to_propagate {
            let constraint = self.constraints[propagator].constraint.as_ref();
            let cause = self.id.cause(propagator);
            constraint.propagate(domains, cause)?;
        }
        Ok(())
    }

    fn explain(&mut self, literal: Lit, context: InferenceCause, state: &Domains, out_explanation: &mut Explanation) {
        let constraint_id = PropagatorId::from(context.payload);
        let constraint = self.constraints[constraint_id].constraint.as_ref();
        constraint.explain(literal, state, out_explanation);
    }

    fn print_stats(&self) {
        println!("# constraints: {}", self.constraints.len())
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
