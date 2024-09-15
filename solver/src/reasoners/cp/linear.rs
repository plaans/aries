// =========== Sum ===========

use crate::core::state::{Cause, Domains, Explanation, InvalidUpdate};
use crate::core::{IntCst, Lit, VarRef, INT_CST_MAX, INT_CST_MIN};
use crate::reasoners::cp::{Propagator, PropagatorId, Watches};
use crate::reasoners::Contradiction;
use num_integer::{div_ceil, div_floor};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
pub(super) struct SumElem {
    pub factor: IntCst,
    pub var: VarRef,
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
        Ok(())
    }
}

impl SumElem {
    fn is_constant(&self) -> bool {
        self.var == VarRef::ONE
    }
}

#[derive(Clone, Debug)]
pub(super) struct LinearSumLeq {
    pub elements: Vec<SumElem>,
    pub ub: IntCst,
    pub active: Lit,
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
    fn get_lower_bound(&self, elem: SumElem, domains: &Domains) -> i64 {
        match elem.factor.cmp(&0) {
            Ordering::Less => domains.ub(elem.var) as i64,
            Ordering::Equal => 0,
            Ordering::Greater => domains.lb(elem.var) as i64,
        }
        .saturating_mul(elem.factor as i64)
    }
    fn get_upper_bound(&self, elem: SumElem, domains: &Domains) -> i64 {
        match elem.factor.cmp(&0) {
            Ordering::Less => domains.lb(elem.var) as i64,
            Ordering::Equal => 0,
            Ordering::Greater => domains.ub(elem.var) as i64,
        }
        .saturating_mul(elem.factor as i64)
    }
    fn set_ub(&self, elem: SumElem, ub: i64, domains: &mut Domains, cause: Cause) -> Result<bool, InvalidUpdate> {
        let var = elem.var;

        match elem.factor.cmp(&0) {
            Ordering::Less => {
                // We need to enforce `ub >= var * factor`  with factor < 0
                // enforce  ub / factor <= var    (note that LHS is rational and need to be rounded to integer
                // equiv to ceil(ub / factor) >= var
                let lb = div_ceil(ub, elem.factor as i64);
                let lb = lb.clamp(INT_CST_MIN as i64, INT_CST_MAX as i64) as i32;
                domains.set_lb(elem.var, lb, cause)
            }
            Ordering::Equal => unreachable!(),
            Ordering::Greater => {
                // We need to enforce `ub >= var * factor`  with factor > 0
                // enforce  ub / factor >= var
                // equiv to floor(ub / factor) >= var
                let ub = div_floor(ub, elem.factor as i64);
                let ub = ub.clamp(INT_CST_MIN as i64, INT_CST_MAX as i64) as i32;
                domains.set_ub(elem.var, ub, cause)
            }
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
        context.add_watch(self.active.variable(), id);
        for e in &self.elements {
            if !e.is_constant() {
                context.add_watch(e.var, id);
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
                .map(|e| self.get_lower_bound(e, domains))
                .sum();
            let f = (self.ub as i64) - sum_lb;

            if f < 0 {
                // INCONSISTENT
                let mut expl = Explanation::new();
                self.explain(Lit::FALSE, domains, &mut expl);
                return Err(Contradiction::Explanation(expl));
            }

            for &e in &self.elements {
                let lb = self.get_lower_bound(e, domains);
                let ub = self.get_upper_bound(e, domains);
                debug_assert!(lb <= ub);
                if ub - lb > f {
                    let new_ub = f + lb;
                    self.set_ub(e, new_ub, domains, cause)?;
                }
            }
        }
        Ok(())
    }

    fn explain(&self, literal: Lit, domains: &Domains, out_explanation: &mut Explanation) {
        if self.active != Lit::TRUE {
            out_explanation.push(self.active);
        }

        for e in &self.elements {
            if e.var != literal.variable() && !e.is_constant() {
                // We are interested with the bounds of the variable only if it may be present in the sum
                // and if it not a constant (i.e. `VarRef::ONE`).
                match e.factor.cmp(&0) {
                    Ordering::Less => out_explanation.push(Lit::leq(e.var, domains.ub(e.var))),
                    Ordering::Equal => {}
                    Ordering::Greater => out_explanation.push(Lit::geq(e.var, domains.lb(e.var))),
                }
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::backtrack::Backtrack;
    use crate::core::{SignedVar, UpperBound};

    use super::*;

    /* ============================== Factories ============================= */

    fn cst(value: IntCst) -> SumElem {
        SumElem {
            factor: value,
            var: VarRef::ONE,
        }
    }

    fn var(lb: IntCst, ub: IntCst, factor: IntCst, dom: &mut Domains) -> SumElem {
        let x = dom.new_var(lb, ub);
        SumElem { factor, var: x }
    }

    fn sum(elements: Vec<SumElem>, ub: IntCst, active: Lit) -> LinearSumLeq {
        LinearSumLeq { elements, ub, active }
    }

    /* =============================== Helpers ============================== */

    fn check_bounds(s: &LinearSumLeq, e: SumElem, d: &Domains, lb: IntCst, ub: IntCst) {
        assert_eq!(s.get_lower_bound(e, d), lb.into());
        assert_eq!(s.get_upper_bound(e, d), ub.into());
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
        let v = var(-100, 100, 2, &mut d);
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
        let c = cst(3);
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
        let x = var(-100, 100, 2, &mut d);
        let c = cst(3);
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
        let x = var(-100, 100, 2, &mut d);
        let y = var(-100, 100, 3, &mut d);
        let z = var(-100, 100, 1, &mut d);
        let c = cst(25);
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
        let x = var(-100, 100, 2, &mut d);
        let y = var(-100, 100, -3, &mut d);
        let z = var(-100, 100, 0, &mut d);
        let c = cst(25);
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
    /// Test that the explanation of an impossible sum `25 <= 10` is its present
    fn test_explanation_present_impossible_sum() {
        let mut d = Domains::new();
        let v = d.new_var(-1, 1);
        let c = cst(25);
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
}
