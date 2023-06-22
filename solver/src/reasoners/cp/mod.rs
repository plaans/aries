#![allow(unused)] // TODO: remove once stabilized

use crate::backtrack::{Backtrack, DecLvl, ObsTrailCursor};
use crate::collections::ref_store::RefVec;
use crate::collections::*;
use crate::core::state::{Cause, Domains, Event, Explanation, InvalidUpdate};
use crate::core::{IntCst, Lit, SignedVar, VarRef, INT_CST_MAX, INT_CST_MIN};
use crate::create_ref_type;
use crate::model::lang::linear::NFLinearLeq;
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use num_integer::{div_ceil, div_floor};
use std::cmp::Ordering;
use std::collections::HashMap;

// =========== Sum ===========

#[derive(Clone, Copy, Debug)]
struct SumElem {
    factor: IntCst,
    /// If None, the var value is considered to be 1
    var: Option<VarRef>,
    /// If true, then the variable should be present. Otherwise, the term is ignored.
    lit: Lit,
}

#[derive(Clone, Debug)]
struct LinearSumLeq {
    elements: Vec<SumElem>,
    ub: IntCst,
    active: Lit,
}

impl LinearSumLeq {
    fn get_lower_bound(&self, elem: SumElem, domains: &Domains) -> IntCst {
        if let Some(var) = elem.var {
            debug_assert!(!domains.entails(elem.lit) || domains.present(var) == Some(true));
            let int_part = match elem.factor.cmp(&0) {
                Ordering::Less => domains
                    .ub(var)
                    .saturating_mul(elem.factor)
                    .clamp(INT_CST_MIN, INT_CST_MAX),
                Ordering::Equal => 0,
                Ordering::Greater => domains
                    .lb(var)
                    .saturating_mul(elem.factor)
                    .clamp(INT_CST_MIN, INT_CST_MAX),
            };
            match domains.present(var) {
                Some(true) => int_part, // note that if there is no default value, the variable is necessarily present
                Some(false) => 0,
                None => 0.min(int_part),
            }
        } else {
            // If None, the var value is considered to be 1
            elem.factor
        }
    }
    fn get_upper_bound(&self, elem: SumElem, domains: &Domains) -> IntCst {
        if let Some(var) = elem.var {
            debug_assert!(!domains.entails(elem.lit) || domains.present(var) == Some(true));
            let int_part = match elem.factor.cmp(&0) {
                Ordering::Less => domains
                    .lb(var)
                    .saturating_mul(elem.factor)
                    .clamp(INT_CST_MIN, INT_CST_MAX),
                Ordering::Equal => 0,
                Ordering::Greater => domains
                    .ub(var)
                    .saturating_mul(elem.factor)
                    .clamp(INT_CST_MIN, INT_CST_MAX),
            };
            match domains.present(var) {
                Some(true) => int_part, // note that if there is no default value, the variable is necessarily present
                Some(false) => 0,
                None => 0.max(int_part),
            }
        } else {
            /// If None, the var value is considered to be 1
            elem.factor
        }
    }
    fn set_ub(&self, elem: SumElem, ub: IntCst, domains: &mut Domains, cause: Cause) -> Result<bool, InvalidUpdate> {
        assert!(elem.var.is_some(), "Cannot set ub for constant variable");
        let var = elem.var.unwrap();
        debug_assert!(!domains.entails(elem.lit) || domains.present(var) == Some(true));
        // println!(
        //     "  {:?} : [{}, {}]    ub: {ub}   -> {}",
        //     var,
        //     domains.lb(var),
        //     domains.ub(var),
        //     ub / elem.factor,
        // );
        match elem.factor.cmp(&0) {
            Ordering::Less => domains.set_lb(var, div_ceil(ub, elem.factor), cause),
            Ordering::Equal => unreachable!(),
            Ordering::Greater => domains.set_ub(var, div_floor(ub, elem.factor), cause),
        }
    }

    fn print(&self, domains: &Domains) {
        println!("ub: {}", self.ub);
        for &e in &self.elements {
            println!(
                " (?{:?}) {:?} x {:?} : [{}, {}]",
                if let Some(var) = e.var {
                    domains.presence(var)
                } else {
                    Lit::TRUE
                },
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
            if let Some(var) = e.var {
                // context.add_watch(VarBound::lb(var), id);
                // context.add_watch(VarBound::ub(var), id);
                match e.factor.cmp(&0) {
                    Ordering::Less => context.add_watch(SignedVar::plus(var), id),
                    Ordering::Equal => {}
                    Ordering::Greater => context.add_watch(SignedVar::minus(var), id),
                }
                // if e.or_zero {
                // TODO: watch presence
                // }
            }
        }
    }
    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        if domains.entails(self.active) {
            // constraint is active, propagate
            let sum_lb: IntCst = self
                .elements
                .iter()
                .copied()
                .filter(|e| !domains.entails(!e.lit))
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
        }
        // println!("AFTER PROP");
        // self.print(domains);
        // println!();
        Ok(())
    }

    fn explain(&self, literal: Lit, domains: &Domains, out_explanation: &mut Explanation) {
        if domains.entails(self.active) {
            for e in &self.elements {
                if let Some(var) = e.var {
                    if var != literal.variable() {
                        match e.factor.cmp(&0) {
                            Ordering::Less => out_explanation.push(Lit::leq(var, domains.ub(var))),
                            Ordering::Equal => {}
                            Ordering::Greater => out_explanation.push(Lit::geq(var, domains.lb(var))),
                        }
                    }
                }
                if e.lit != Lit::TRUE {
                    match domains.value(e.lit) {
                        Some(true) => out_explanation.push(e.lit),
                        Some(false) => out_explanation.push(!e.lit),
                        _ => {}
                    }
                }
            }
            // dbg!(&self);
            // dbg!(&out_explanation.lits);
        }
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
    propagations: HashMap<SignedVar, Vec<PropagatorId>>,
    empty: [PropagatorId; 0],
}

impl Watches {
    fn add_watch(&mut self, watched: SignedVar, propagator_id: PropagatorId) {
        self.propagations
            .entry(watched)
            .or_insert_with(|| Vec::with_capacity(4))
            .push(propagator_id)
    }

    fn get(&self, var_bound: SignedVar) -> &[PropagatorId] {
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
        let elements = leq
            .sum
            .iter()
            .map(|e| SumElem {
                factor: e.factor,
                var: e.var,
                lit: e.lit,
            })
            .collect();
        let propagator = LinearSumLeq {
            elements,
            ub: leq.upper_bound,
            active,
        };
        self.add_propagator(propagator);
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
    fn identity(&self) -> ReasonerId {
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

// impl BindSplit for Cp {
//     fn enforce_true(&mut self, expr: &Expr, _doms: &mut Domains) -> BindingResult {
//         if let Some(leq) = downcast::<NFLinearLeq>(expr) {
//             let elements = leq
//                 .sum
//                 .iter()
//                 .map(|e| SumElem {
//                     factor: e.factor,
//                     var: e.var,
//                     or_zero: e.or_zero,
//                 })
//                 .collect();
//             let propagator = LinearSumLeq {
//                 elements,
//                 ub: leq.upper_bound,
//             };
//             self.add_propagator(propagator);
//             BindingResult::Enforced
//         } else {
//             BindingResult::Unsupported
//         }
//     }
//
//     fn enforce_false(&mut self, _expr: &Expr, _doms: &mut Domains) -> BindingResult {
//         // TODO
//         BindingResult::Unsupported
//     }
//
//     fn enforce_eq(&mut self, _literal: Lit, _expr: &Expr, _doms: &mut Domains) -> BindingResult {
//         // TODO
//         BindingResult::Unsupported
//     }
// }

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    /* ================================= Sum ================================ */

    /// Returns the constraint `x + 2 <= 0`
    fn simple_constraint() -> (LinearSumLeq, Domains) {
        let mut dom = Domains::new();
        let x = dom.new_var(-100, 100);
        (
            LinearSumLeq {
                elements: vec![
                    SumElem {
                        factor: 1,
                        var: Some(x),
                        lit: Lit::TRUE,
                    },
                    SumElem {
                        factor: 2,
                        var: None,
                        lit: Lit::TRUE,
                    },
                ],
                ub: 0,
                active: Lit::TRUE,
            },
            dom,
        )
    }

    #[test]
    fn get_lower_bound() {
        let (l, d) = simple_constraint();
        let x = l.elements.get(0).unwrap();
        let c = l.elements.get(1).unwrap();
        assert_eq!(l.get_lower_bound(*x, &d), -100);
        assert_eq!(l.get_lower_bound(*c, &d), 2);
    }

    #[test]
    fn get_upper_bound() {
        let (l, d) = simple_constraint();
        let x = l.elements.get(0).unwrap();
        let c = l.elements.get(1).unwrap();
        assert_eq!(l.get_upper_bound(*x, &d), 100);
        assert_eq!(l.get_upper_bound(*c, &d), 2);
    }

    #[test]
    fn set_ub_variable() {
        let (l, mut d) = simple_constraint();
        let x = l.elements.get(0).unwrap();
        assert_eq!(l.get_upper_bound(*x, &d), 100);
        l.set_ub(*x, 50, &mut d, Cause::Decision);
        assert_eq!(l.get_upper_bound(*x, &d), 50);
    }

    #[test]
    #[should_panic(expected = "Cannot set ub for constant variable")]
    fn set_ub_constant() {
        let (l, mut d) = simple_constraint();
        let c = l.elements.get(1).unwrap();
        assert_eq!(l.get_upper_bound(*c, &d), 2);
        l.set_ub(*c, 5, &mut d, Cause::Decision);
    }

    #[test]
    fn propagate() {
        let (l, mut d) = simple_constraint();
        let x = l.elements.get(0).unwrap();
        let c = l.elements.get(1).unwrap();
        l.propagate(&mut d, Cause::Decision);
        assert_eq!(l.get_lower_bound(*x, &d), -100);
        assert_eq!(l.get_upper_bound(*x, &d), -2);
        assert_eq!(l.get_lower_bound(*c, &d), 2);
        assert_eq!(l.get_upper_bound(*c, &d), 2);
    }
}
