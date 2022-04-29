#![allow(unused)] // TODO: remove once stabilized

use aries_backtrack::{Backtrack, DecLvl, ObsTrailCursor};
use aries_collections::ref_store::RefVec;
use aries_collections::*;
use aries_core::state::{Cause, Domains, Event, Explanation, InvalidUpdate};
use aries_core::{IntCst, Lit, VarBound, VarRef, WriterId};
use aries_model::lang::linear::NFLinearLeq;
use aries_model::lang::reification::{downcast, Expr};
use aries_solver::solver::BindingResult;
use aries_solver::{BindSplit, Contradiction, Theory};
use num_integer::{div_ceil, div_floor};
use std::cmp::Ordering;
use std::collections::HashMap;

// =========== Sum ===========

#[derive(Clone, Copy, Debug)]
struct SumElem {
    factor: IntCst,
    var: VarRef,
    // if true, this element should be evaluated to zero if the variable is empty.
    or_zero: bool,
}

#[derive(Clone, Debug)]
struct LinearSumLeq {
    elements: Vec<SumElem>,
    ub: IntCst,
}

impl LinearSumLeq {
    fn get_lower_bound(&self, elem: SumElem, domains: &Domains) -> IntCst {
        debug_assert!(elem.or_zero || domains.present(elem.var) == Some(true));
        let int_part = match elem.factor.cmp(&0) {
            Ordering::Less => domains.ub(elem.var) * elem.factor,
            Ordering::Equal => 0,
            Ordering::Greater => domains.lb(elem.var) * elem.factor,
        };
        match domains.present(elem.var) {
            Some(true) => int_part, // note that if there is no default value, the variable is necessarily present
            Some(false) => 0,
            None => 0.min(int_part),
        }
    }
    fn get_upper_bound(&self, elem: SumElem, domains: &Domains) -> IntCst {
        debug_assert!(elem.or_zero || domains.present(elem.var) == Some(true));
        let int_part = match elem.factor.cmp(&0) {
            Ordering::Less => domains.lb(elem.var) * elem.factor,
            Ordering::Equal => 0,
            Ordering::Greater => domains.ub(elem.var) * elem.factor,
        };
        match domains.present(elem.var) {
            Some(true) => int_part, // note that if there is no default value, the variable is necessarily present
            Some(false) => 0,
            None => 0.max(int_part),
        }
    }
    fn set_ub(&self, elem: SumElem, ub: IntCst, domains: &mut Domains, cause: Cause) -> Result<bool, InvalidUpdate> {
        debug_assert!(elem.or_zero || domains.present(elem.var) == Some(true));
        // println!(
        //     "  {:?} : [{}, {}]    ub: {ub}   -> {}",
        //     elem.var,
        //     domains.lb(elem.var),
        //     domains.ub(elem.var),
        //     ub / elem.factor,
        // );
        match elem.factor.cmp(&0) {
            Ordering::Less => domains.set_lb(elem.var, div_ceil(ub, elem.factor), cause),
            Ordering::Equal => unreachable!(),
            Ordering::Greater => domains.set_ub(elem.var, div_floor(ub, elem.factor), cause),
        }
    }

    fn print(&self, domains: &Domains) {
        println!("ub: {}", self.ub);
        for &e in &self.elements {
            println!(
                " (?{:?}) {:?} x {:?} : [{}, {}]",
                domains.presence(e.var),
                e.factor,
                e.var,
                self.get_lower_bound(e, domains),
                self.get_upper_bound(e, domains)
            )
        }
    }
}

impl Propagator for LinearSumLeq {
    fn setup(&self, id: PropagatorId, context: &mut Watches) {
        // println!("SET UP");

        for e in &self.elements {
            // context.add_watch(VarBound::lb(e.var), id);
            // context.add_watch(VarBound::ub(e.var), id);
            match e.factor.cmp(&0) {
                Ordering::Less => context.add_watch(VarBound::ub(e.var), id),
                Ordering::Equal => {}
                Ordering::Greater => context.add_watch(VarBound::lb(e.var), id),
            }
            if e.or_zero {
                // TODO: watch presence
            }
        }
    }
    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        let sum_lb: IntCst = self
            .elements
            .iter()
            .copied()
            .map(|e| self.get_lower_bound(e, domains))
            .sum();
        let f = self.ub - sum_lb;
        // println!("Propagation : {} <= {}", sum_lb, self.ub);
        // self.print(domains);
        if f < 0 {
            // println!("INCONSISTENT");
            let mut expl = Explanation::new();
            self.explain(Lit::FALSE, domains, &mut expl);
            return Err(Contradiction::Explanation(expl));
        }
        for &e in &self.elements {
            let lb = self.get_lower_bound(e, domains);
            let ub = self.get_upper_bound(e, domains);
            debug_assert!(lb <= ub);
            if ub - lb > f {
                // println!("  problem on: {e:?} {lb} {ub}");
                match self.set_ub(e, f + lb, domains, cause) {
                    Ok(true) => {} // println!("    propagated: {e:?} <= {}", f + lb),
                    Ok(false) => {}
                    Err(e) => {
                        // println!("    invalid update");
                        return Err(e.into());
                    }
                }
            }
        }
        // println!("AFTER PROP");
        // self.print(domains);
        // println!();
        Ok(())
    }

    fn explain(&self, literal: Lit, domains: &Domains, out_explanation: &mut Explanation) {
        for e in &self.elements {
            if e.var != literal.variable() {
                match e.factor.cmp(&0) {
                    Ordering::Less => out_explanation.push(Lit::leq(e.var, domains.ub(e.var))),
                    Ordering::Equal => {}
                    Ordering::Greater => out_explanation.push(Lit::geq(e.var, domains.lb(e.var))),
                }
            }
            if e.or_zero {
                let prez = domains.presence(e.var);
                // println!("{prez:?}, {:?}", domains.value(prez));
                match domains.value(prez) {
                    Some(true) => out_explanation.push(prez),
                    Some(false) => out_explanation.push(!prez),
                    _ => {}
                }
            }
        }
        // dbg!(&self);
        // dbg!(&out_explanation.lits);
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

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

struct DynPropagator {
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
    propagations: HashMap<VarBound, Vec<PropagatorId>>,
    empty: [PropagatorId; 0],
}

impl Watches {
    fn add_watch(&mut self, watched: VarBound, propagator_id: PropagatorId) {
        self.propagations
            .entry(watched)
            .or_insert_with(|| Vec::with_capacity(4))
            .push(propagator_id)
    }

    fn get(&self, var_bound: VarBound) -> &[PropagatorId] {
        self.propagations
            .get(&var_bound)
            .map(|v| v.as_slice())
            .unwrap_or(&self.empty)
    }
}

#[derive(Clone)]
pub struct Cp {
    id: WriterId,
    constraints: RefVec<PropagatorId, DynPropagator>,
    model_events: ObsTrailCursor<Event>,
    watches: Watches,
    saved: DecLvl,
}

impl Cp {
    pub fn new(id: WriterId) -> Cp {
        Cp {
            id,
            constraints: Default::default(),
            model_events: ObsTrailCursor::new(),
            watches: Default::default(),
            saved: DecLvl::ROOT,
        }
    }

    fn add_propagator(&mut self, propagator: impl Into<DynPropagator>) {
        // TODO: handle validity scopes
        let propagator = propagator.into();
        let propagator_id = self.constraints.next_key();
        propagator.constraint.setup(propagator_id, &mut self.watches);
        self.constraints.set_next(propagator_id, propagator);
    }
}

impl Theory for Cp {
    fn identity(&self) -> WriterId {
        self.id
    }

    fn propagate(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        // TODO: at this point, all propagators are invoked regardless of watches
        // if self.saved == DecLvl::ROOT {
        for (id, p) in self.constraints.entries() {
            let cause = self.id.cause(id);
            p.constraint.propagate(domains, cause)?;
        }
        // }
        // while let Some(event) = self.model_events.pop(domains.trail()).copied() {
        //     let watchers = self.watches.get(event.affected_bound);
        //     for &watcher in watchers {
        //         let constraint = self.constraints[watcher].constraint.as_ref();
        //         let cause = self.id.cause(watcher);
        //         constraint.propagate(&event, domains, cause)?;
        //     }
        // }
        Ok(())
    }

    fn explain(&mut self, literal: Lit, context: u32, state: &Domains, out_explanation: &mut Explanation) {
        let constraint_id = PropagatorId::from(context);
        let constraint = self.constraints[constraint_id].constraint.as_ref();
        constraint.explain(literal, state, out_explanation);
    }

    fn print_stats(&self) {
        // TODO: print some statistics
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

impl BindSplit for Cp {
    fn enforce_true(&mut self, expr: &Expr, _doms: &mut Domains) -> BindingResult {
        if let Some(leq) = downcast::<NFLinearLeq>(expr) {
            let elements = leq
                .sum
                .iter()
                .map(|e| SumElem {
                    factor: e.factor,
                    var: e.var,
                    or_zero: e.or_zero,
                })
                .collect();
            let propagator = LinearSumLeq {
                elements,
                ub: leq.upper_bound,
            };
            self.add_propagator(propagator);
            BindingResult::Enforced
        } else {
            BindingResult::Unsupported
        }
    }

    fn enforce_false(&mut self, _expr: &Expr, _doms: &mut Domains) -> BindingResult {
        todo!()
    }

    fn enforce_eq(&mut self, _literal: Lit, _expr: &Expr, _doms: &mut Domains) -> BindingResult {
        todo!()
    }
}
