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
        let var_is_present = elem.var.map_or(Some(true), |v| domains.present(v)) == Some(true);
        debug_assert!(!domains.entails(elem.lit) || var_is_present);

        let int_part = match elem.var {
            Some(var) => match elem.factor.cmp(&0) {
                Ordering::Less => domains.ub(var),
                Ordering::Equal => 0,
                Ordering::Greater => domains.lb(var),
            },
            None => 1,
        }
        .saturating_mul(elem.factor)
        .clamp(INT_CST_MIN, INT_CST_MAX);

        match domains.value(elem.lit) {
            Some(true) => int_part,
            Some(false) => 0,
            None => 0.min(int_part),
        }
    }
    fn get_upper_bound(&self, elem: SumElem, domains: &Domains) -> IntCst {
        let var_is_present = elem.var.map_or(Some(true), |v| domains.present(v)) == Some(true);
        debug_assert!(!domains.entails(elem.lit) || var_is_present);

        let int_part = match elem.var {
            Some(var) => match elem.factor.cmp(&0) {
                Ordering::Less => domains.lb(var),
                Ordering::Equal => 0,
                Ordering::Greater => domains.ub(var),
            },
            None => 1,
        }
        .saturating_mul(elem.factor)
        .clamp(INT_CST_MIN, INT_CST_MAX);

        match domains.value(elem.lit) {
            Some(true) => int_part,
            Some(false) => 0,
            None => 0.max(int_part),
        }
    }
    fn set_ub(&self, elem: SumElem, ub: IntCst, domains: &mut Domains, cause: Cause) -> Result<bool, InvalidUpdate> {
        if elem.var.is_none() && elem.factor == ub {
            // Try to change the upper bound of a constant but the upper bound is its current value.
            return Ok(false);
        }
        assert!(
            elem.var.is_some(),
            "Try to set {ub} as upper bound of the constant {elem:?}"
        );

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
        out_explanation.push(self.active);
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

    /* ============================== Factories ============================= */

    fn cst(value: IntCst, lit: Lit) -> SumElem {
        SumElem {
            factor: value,
            var: None,
            lit,
        }
    }

    fn var(lb: IntCst, ub: IntCst, factor: IntCst, lit: Lit, dom: &mut Domains) -> SumElem {
        let x = dom.new_var(lb, ub);
        SumElem {
            factor,
            var: Some(x),
            lit,
        }
    }

    fn sum(elements: Vec<SumElem>, ub: IntCst, active: Lit) -> LinearSumLeq {
        LinearSumLeq { elements, ub, active }
    }

    /* =============================== Helpers ============================== */

    fn check_bounds(s: &LinearSumLeq, e: &SumElem, d: &Domains, lb: IntCst, ub: IntCst) {
        assert_eq!(s.get_lower_bound(*e, d), lb);
        assert_eq!(s.get_upper_bound(*e, d), ub);
    }

    /* ================================ Tests =============================== */

    #[test]
    /// Tests that the upper bound of a variable can be changed
    fn test_ub_setter_var() {
        let mut d = Domains::new();
        let v = var(-100, 100, 2, Lit::TRUE, &mut d);
        let s = sum(vec![v], 10, Lit::TRUE);
        check_bounds(&s, &v, &d, -200, 200);
        assert_eq!(s.set_ub(v, 50, &mut d, Cause::Decision), Ok(true));
        check_bounds(&s, &v, &d, -200, 50);
    }

    #[test]
    #[should_panic]
    /// Tests that the upper bound of a constant cannot be changed
    fn test_ub_setter_cst() {
        let mut d = Domains::new();
        let c = cst(3, Lit::TRUE);
        let s = sum(vec![c], 10, Lit::TRUE);
        check_bounds(&s, &c, &d, 3, 3);
        s.set_ub(c, 50, &mut d, Cause::Decision);
    }

    #[test]
    /// Tests that setting the upper bound of a constant doesn't panic if it is its current value
    fn test_ub_setter_cst_unchanged() {
        let mut d = Domains::new();
        let c = cst(3, Lit::TRUE);
        let s = sum(vec![c], 10, Lit::TRUE);
        check_bounds(&s, &c, &d, 3, 3);
        assert_eq!(s.set_ub(c, 3, &mut d, Cause::Decision), Ok(false));
        check_bounds(&s, &c, &d, 3, 3);
    }

    #[test]
    /// Tests on the constraint `2*x + 3 <= 10` with `x` in `[-100, 100]`
    fn test_single_var_constraint() {
        let mut d = Domains::new();
        let x = var(-100, 100, 2, Lit::TRUE, &mut d);
        let c = cst(3, Lit::TRUE);
        let s = sum(vec![x, c], 10, Lit::TRUE);

        // Check bounds
        check_bounds(&s, &x, &d, -200, 200);
        check_bounds(&s, &c, &d, 3, 3);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(&s, &x, &d, -200, 6); // We should have an upper bound of 7 but `x` is an integer so we have `x=floor(7/2)*2`
        check_bounds(&s, &c, &d, 3, 3);
    }

    #[test]
    /// Tests on the constraint `2*x + 3*y + z + 25 <= 10` with variables in `[-100, 100]`
    fn test_multi_var_constraint() {
        let mut d = Domains::new();
        let x = var(-100, 100, 2, Lit::TRUE, &mut d);
        let y = var(-100, 100, 3, Lit::TRUE, &mut d);
        let z = var(-100, 100, 1, Lit::TRUE, &mut d);
        let c = cst(25, Lit::TRUE);
        let s = sum(vec![x, y, z, c], 10, Lit::TRUE);

        // Check bounds
        check_bounds(&s, &x, &d, -200, 200);
        check_bounds(&s, &y, &d, -300, 300);
        check_bounds(&s, &z, &d, -100, 100);
        check_bounds(&s, &c, &d, 25, 25);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(&s, &x, &d, -200, 200);
        check_bounds(&s, &y, &d, -300, 285);
        check_bounds(&s, &z, &d, -100, 100);
        check_bounds(&s, &c, &d, 25, 25);
    }
}
