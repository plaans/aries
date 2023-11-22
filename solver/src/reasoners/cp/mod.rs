#![allow(unused)] // TODO: remove once stabilized

use crate::backtrack::{Backtrack, DecLvl, ObsTrailCursor};
use crate::collections::ref_store::RefVec;
use crate::collections::*;
use crate::core::state::{Cause, Domains, Event, Explanation, InvalidUpdate};
use crate::core::{IntCst, Lit, SignedVar, VarRef, INT_CST_MAX, INT_CST_MIN};
use crate::create_ref_type;
use crate::model::lang::linear::NFLinearLeq;
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use anyhow::Context;
use num_integer::{div_ceil, div_floor};
use std::cmp::Ordering;
use std::collections::HashMap;

// =========== Sum ===========

#[derive(Clone, Copy, Debug)]
struct SumElem {
    factor: IntCst,
    var: VarRef,
    /// If true, then the variable should be present. Otherwise, the term is ignored.
    lit: Lit,
}

impl std::fmt::Display for SumElem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.factor != 1 {
            if self.factor < 0 {
                write!(f, "({})", self.factor)?;
            } else {
                write!(f, "{}", self.factor)?;
            }
            write!(f, "*")?;
        }
        if self.var != VarRef::ONE {
            write!(f, "{:?}", self.var)?;
        }
        write!(f, "[{:?}]", self.lit)
    }
}

impl SumElem {
    fn is_constant(&self) -> bool {
        self.var == VarRef::ONE
    }
}

#[derive(Clone, Debug)]
struct LinearSumLeq {
    elements: Vec<SumElem>,
    ub: IntCst,
    active: Lit,
}

impl std::fmt::Display for LinearSumLeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prez = format!("[{:?}]", self.active);
        write!(f, "{prez:<8}")?;
        for (i, e) in self.elements.iter().enumerate() {
            if i != 0 {
                write!(f, " + ")?;
            }
            write!(f, "{e}")?;
        }
        write!(f, " <= {}", self.ub)
    }
}

impl LinearSumLeq {
    fn get_lower_bound(&self, elem: SumElem, domains: &Domains) -> IntCst {
        let var_is_present = domains.present(elem.var) == Some(true);
        debug_assert!(!domains.entails(elem.lit) || var_is_present);

        let int_part = match elem.factor.cmp(&0) {
            Ordering::Less => domains.ub(elem.var),
            Ordering::Equal => 0,
            Ordering::Greater => domains.lb(elem.var),
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
        let var_is_present = domains.present(elem.var) == Some(true);
        debug_assert!(!domains.entails(elem.lit) || var_is_present);

        let int_part = match elem.factor.cmp(&0) {
            Ordering::Less => domains.lb(elem.var),
            Ordering::Equal => 0,
            Ordering::Greater => domains.ub(elem.var),
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
        debug_assert!(!domains.entails(elem.lit) || domains.present(elem.var) == Some(true));
        // println!(
        //     "  {:?} : [{}, {}]    ub: {ub}   -> {}",
        //     var,
        //     domains.lb(var),
        //     domains.ub(var),
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
            if !e.is_constant() {
                context.add_watch(SignedVar::plus(e.var), id);
                context.add_watch(SignedVar::minus(e.var), id);
            }
            if e.lit != Lit::TRUE {
                context.add_watch(e.lit.svar(), id);
                context.add_watch(e.lit.svar().neg(), id);
            }
        }
    }
    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        if domains.entails(self.active) {
            // constraint is active, propagate
            let sum_lb: i64 = self
                .elements
                .iter()
                .copied()
                .filter(|e| !domains.entails(!e.lit))
                .map(|e| self.get_lower_bound(e, domains) as i64)
                .sum();
            let f = (self.ub as i64) - sum_lb;
            // println!("Propagation : {} <= {}", sum_lb, self.ub);
            // self.print(domains);
            if f < 0 {
                // println!("INCONSISTENT");
                let mut expl = Explanation::new();
                self.explain(Lit::FALSE, domains, &mut expl);
                return Err(Contradiction::Explanation(expl));
            }
            for &e in &self.elements {
                let lb = self.get_lower_bound(e, domains) as i64;
                let ub = self.get_upper_bound(e, domains) as i64;
                debug_assert!(lb <= ub);
                if ub - lb > f {
                    // println!("  problem on: {e:?} {lb} {ub}");
                    // NOTE: Conversion from i64 to i32 should not fail due to the clamp between two i32 values.
                    let new_ub = (f + lb).clamp(INT_CST_MIN as i64, INT_CST_MAX as i64) as i32;
                    match self.set_ub(e, new_ub, domains, cause) {
                        Ok(true) => {} // println!("    propagated: {e:?} <= {}", f + lb),
                        Ok(false) => {}
                        Err(err) => {
                            // If the update is invalid, a solution could be to force the element to not be present.
                            if !domains.entails(e.lit) {
                                match domains.set(!e.lit, cause) {
                                    Ok(_) => {}
                                    Err(err2) => {
                                        return Err(err2.into());
                                    }
                                }
                            } else {
                                return Err(err.into());
                            }
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
        if self.active != Lit::TRUE {
            out_explanation.push(self.active);
        }

        for e in &self.elements {
            if e.var != literal.variable() && !domains.entails(!e.lit) && !e.is_constant() {
                // We are interested with the bounds of the variable only if it may be present in the sum
                // and if it not a constant (i.e. `VarRef::ONE`).
                match e.factor.cmp(&0) {
                    Ordering::Less => out_explanation.push(Lit::leq(e.var, domains.ub(e.var))),
                    Ordering::Equal => {}
                    Ordering::Greater => out_explanation.push(Lit::geq(e.var, domains.lb(e.var))),
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
        if self.saved == DecLvl::ROOT {
            // in first propagation, propagate everyone
            for (id, p) in self.constraints.entries() {
                let cause = self.id.cause(id);
                p.constraint.propagate(domains, cause)?;
            }
        }
        while let Some(event) = self.model_events.pop(domains.trail()).copied() {
            let watchers = self.watches.get(event.affected_bound);
            for &watcher in watchers {
                let constraint = self.constraints[watcher].constraint.as_ref();
                let cause = self.id.cause(watcher);
                constraint.propagate(domains, cause)?;
            }
        }
        Ok(())
    }

    fn explain(&mut self, literal: Lit, context: u32, state: &Domains, out_explanation: &mut Explanation) {
        let constraint_id = PropagatorId::from(context);
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
    use crate::core::UpperBound;

    use super::*;

    /* ============================== Factories ============================= */

    fn cst(value: IntCst, lit: Lit) -> SumElem {
        SumElem {
            factor: value,
            var: VarRef::ONE,
            lit,
        }
    }

    fn var(lb: IntCst, ub: IntCst, factor: IntCst, lit: Lit, dom: &mut Domains) -> SumElem {
        let x = dom.new_var(lb, ub);
        SumElem { factor, var: x, lit }
    }

    fn sum(elements: Vec<SumElem>, ub: IntCst, active: Lit) -> LinearSumLeq {
        LinearSumLeq { elements, ub, active }
    }

    /* =============================== Helpers ============================== */

    fn check_bounds(s: &LinearSumLeq, e: SumElem, d: &Domains, lb: IntCst, ub: IntCst) {
        assert_eq!(s.get_lower_bound(e, d), lb);
        assert_eq!(s.get_upper_bound(e, d), ub);
    }

    fn check_bounds_var(v: VarRef, d: &Domains, lb: IntCst, ub: IntCst) {
        assert_eq!(d.lb(v), lb);
        assert_eq!(d.ub(v), ub);
    }

    /* ================================ Tests =============================== */

    #[test]
    /// Tests that the upper bound of a variable can be changed
    fn test_ub_setter_var() {
        let mut d = Domains::new();
        let v = var(-100, 100, 2, Lit::TRUE, &mut d);
        let s = sum(vec![v], 10, Lit::TRUE);
        check_bounds(&s, v, &d, -200, 200);
        assert_eq!(s.set_ub(v, 50, &mut d, Cause::Decision), Ok(true));
        check_bounds(&s, v, &d, -200, 50);
        assert_eq!(s.set_ub(v, 50, &mut d, Cause::Decision), Ok(false));
        check_bounds(&s, v, &d, -200, 50);
    }

    #[test]
    /// Tests that the upper bound of a constant can be changed if it is greater or equal to the current value
    fn test_ub_setter_cst() {
        let mut d = Domains::new();
        let c = cst(3, Lit::TRUE);
        let s = sum(vec![c], 10, Lit::TRUE);
        check_bounds(&s, c, &d, 3, 3);
        assert_eq!(s.set_ub(c, 50, &mut d, Cause::Decision), Ok(false));
        check_bounds(&s, c, &d, 3, 3);
        assert_eq!(s.set_ub(c, 3, &mut d, Cause::Decision), Ok(false));
        check_bounds(&s, c, &d, 3, 3);
        assert_eq!(
            s.set_ub(c, 0, &mut d, Cause::Decision),
            Err(InvalidUpdate(
                Lit::from_parts(SignedVar::plus(VarRef::ONE), UpperBound::ub(0)),
                Cause::Decision.into()
            ))
        );
        check_bounds(&s, c, &d, 3, 3);
    }

    #[test]
    /// Tests on the constraint `2*x + 3 <= 10` with `x` in `[-100, 100]`
    fn test_single_var_constraint() {
        let mut d = Domains::new();
        let x = var(-100, 100, 2, Lit::TRUE, &mut d);
        let c = cst(3, Lit::TRUE);
        let s = sum(vec![x, c], 10, Lit::TRUE);

        // Check bounds
        check_bounds(&s, x, &d, -200, 200);
        check_bounds(&s, c, &d, 3, 3);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(&s, x, &d, -200, 6); // We should have an upper bound of 7 but `x` is an integer so we have `x=floor(7/2)*2`
        check_bounds(&s, c, &d, 3, 3);

        // Possible ub setting
        assert_eq!(s.set_ub(x, 5, &mut d, Cause::Decision), Ok(true));
        check_bounds(&s, x, &d, -200, 4);
        check_bounds(&s, c, &d, 3, 3);

        // Impossible ub setting
        assert_eq!(s.set_ub(x, 10, &mut d, Cause::Decision), Ok(false));
        check_bounds(&s, x, &d, -200, 4);
        check_bounds(&s, c, &d, 3, 3);
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
        check_bounds(&s, x, &d, -200, 200);
        check_bounds(&s, y, &d, -300, 300);
        check_bounds(&s, z, &d, -100, 100);
        check_bounds(&s, c, &d, 25, 25);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(&s, x, &d, -200, 200);
        check_bounds(&s, y, &d, -300, 285);
        check_bounds(&s, z, &d, -100, 100);
        check_bounds(&s, c, &d, 25, 25);
    }

    #[test]
    /// Tests on the constraint `2*x - 3*y + 0*z + 25 <= 10` with variables in `[-100, 100]`
    fn test_neg_factor_constraint() {
        let mut d = Domains::new();
        let x = var(-100, 100, 2, Lit::TRUE, &mut d);
        let y = var(-100, 100, -3, Lit::TRUE, &mut d);
        let z = var(-100, 100, 0, Lit::TRUE, &mut d);
        let c = cst(25, Lit::TRUE);
        let s = sum(vec![x, y, z, c], 10, Lit::TRUE);

        // Check bounds
        check_bounds(&s, x, &d, -200, 200);
        check_bounds(&s, y, &d, -300, 300);
        check_bounds(&s, z, &d, 0, 0);
        check_bounds(&s, c, &d, 25, 25);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(&s, x, &d, -200, 200);
        check_bounds(&s, y, &d, -300, 183);
        check_bounds(&s, z, &d, 0, 0);
        check_bounds(&s, c, &d, 25, 25);
    }

    #[test]
    /// Tests on the constraint `2*x + y + 25 <= 10` with variables in `[-100, 100]` and literals != true
    fn test_literals_constraint() {
        let mut d = Domains::new();
        let v = d.new_var(-100, 100);
        let x = var(-100, 100, 2, v.lt(0), &mut d);
        let y = var(-100, 100, 1, v.gt(0), &mut d);
        let c = cst(25, Lit::TRUE);
        let s = sum(vec![x, y, c], 10, Lit::TRUE);

        let init_state = d.save_state();
        let set_val = |dom: &mut Domains, val: IntCst| {
            // Reset
            while dom.last_event().is_some() {
                dom.undo_last_event();
            }
            check_bounds_var(v, dom, -100, 100);
            check_bounds(&s, x, dom, -200, 200);
            check_bounds(&s, y, dom, -100, 100);
            check_bounds(&s, c, dom, 25, 25);
            // Set the new value
            dom.set_lb(v, val, Cause::Decision);
            dom.set_ub(v, val, Cause::Decision);
            check_bounds_var(v, dom, val, val);
        };

        // Check bounds
        check_bounds_var(v, &d, -100, 100);
        check_bounds(&s, x, &d, -200, 200);
        check_bounds(&s, y, &d, -100, 100);
        check_bounds(&s, c, &d, 25, 25);

        // Check propagation with `v in [-100, 100]`
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds_var(v, &d, -100, 100);
        check_bounds(&s, x, &d, -200, 84);
        check_bounds(&s, y, &d, -100, 100);
        check_bounds(&s, c, &d, 25, 25);

        // Check propagation with `v < 0`
        set_val(&mut d, -1);
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds_var(v, &d, -1, -1);
        check_bounds(&s, x, &d, -200, -16);
        check_bounds(&s, y, &d, 0, 0);
        check_bounds(&s, c, &d, 25, 25);

        // Check propagation with `v > 0`
        set_val(&mut d, 1);
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds_var(v, &d, 1, 1);
        check_bounds(&s, x, &d, 0, 0);
        check_bounds(&s, y, &d, -100, -15);
        check_bounds(&s, c, &d, 25, 25);

        // Check propagation with `v = 0`
        set_val(&mut d, 0);
        let p = s.propagate(&mut d, Cause::Decision);
        assert!(p.is_err());
        let Contradiction::Explanation(e) = p.unwrap_err() else {
            unreachable!()
        };
        let expected_e: Vec<Lit> = vec![
            v.geq(0), // v must be negative for x to be present
            v.leq(0), // v must be positive for y to be present
        ];
        assert_eq!(e.lits, expected_e);
        check_bounds_var(v, &d, 0, 0);
        check_bounds(&s, x, &d, 0, 0);
        check_bounds(&s, y, &d, 0, 0);
        check_bounds(&s, c, &d, 25, 25);
    }

    #[test]
    /// Test that the explanation of an impossible sum `25 <= 10` is its present
    fn test_explanation_present_impossible_sum() {
        let mut d = Domains::new();
        let v = d.new_var(-1, 1);
        let c = cst(25, Lit::TRUE);
        let s = sum(vec![c], 10, v.lt(0));

        // The sum is not necessary active so everything is ok
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds_var(v, &d, -1, 1);

        // Change the value of `v` to activate the impossible sum
        d.set_lb(v, -1, Cause::Decision);
        d.set_ub(v, -1, Cause::Decision);
        check_bounds_var(v, &d, -1, -1);
        let p = s.propagate(&mut d, Cause::Decision);
        assert!(p.is_err());
        let Contradiction::Explanation(e) = p.unwrap_err() else {
            unreachable!()
        };
        assert_eq!(e.lits, vec![v.lt(0)]);
        check_bounds_var(v, &d, -1, -1);
    }

    #[test]
    /// Test explanation based on the presence and the bounds of a variable
    /// The constraint is `y <= 10` with `y` in `[25, 50]`
    fn test_explanation_pos_var_bounds() {
        let mut d = Domains::new();
        let v = d.new_var(-1, -1);
        let y = var(25, 50, 1, v.lt(0), &mut d);
        let s = sum(vec![y], 10, Lit::TRUE);

        // Check bounds
        check_bounds_var(v, &d, -1, -1);
        check_bounds(&s, y, &d, 25, 50);

        // Check propagation
        let p = s.propagate(&mut d, Cause::Decision);
        assert!(p.is_err());
        let Contradiction::Explanation(e) = p.unwrap_err() else {
            unreachable!()
        };
        assert_eq!(e.lits, vec![y.var.geq(25), v.lt(0)]);
        check_bounds_var(v, &d, -1, -1);
    }

    #[test]
    /// Test explanation based on the presence and the bounds of a variable
    /// The constraint is `-y <= 10` with `y` in `[-50, -25]`
    fn test_explanation_neg_var_bounds() {
        let mut d = Domains::new();
        let v = d.new_var(-1, -1);
        let y = var(-50, -25, -1, v.lt(0), &mut d);
        let s = sum(vec![y], 10, Lit::TRUE);

        // Check bounds
        check_bounds_var(v, &d, -1, -1);
        check_bounds(&s, y, &d, 25, 50);

        // Check propagation
        let p = s.propagate(&mut d, Cause::Decision);
        assert!(p.is_err());
        let Contradiction::Explanation(e) = p.unwrap_err() else {
            unreachable!()
        };
        assert_eq!(e.lits, vec![y.var.leq(-25), v.lt(0)]);
        check_bounds_var(v, &d, -1, -1);
    }

    #[test]
    /// Test that the propagation force an element to be non-present if its bounds cannot be updated
    /// The constraint is `x + 5 <= 10` with `x` in `[25, 50]`
    fn test_propagation_force_non_present() {
        let mut d = Domains::new();
        let v = d.new_var(-1, 1);
        let x = var(25, 50, 1, v.lt(0), &mut d);
        let c = cst(5, v.gt(0));
        let s = sum(vec![x, c], 10, Lit::TRUE);

        // Check bounds
        check_bounds_var(v, &d, -1, 1);
        check_bounds(&s, x, &d, 0, 50);
        check_bounds(&s, c, &d, 0, 5);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds_var(v, &d, 0, 1);
        check_bounds(&s, x, &d, 0, 0);
        check_bounds(&s, c, &d, 0, 5);
    }
}
