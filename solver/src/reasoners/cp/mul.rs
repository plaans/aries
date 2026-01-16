use crate::{
    core::{
        IntCst, Lit, VarRef,
        state::{Cause, Domains, DomainsSnapshot, Explanation},
    },
    reasoners::{
        Contradiction,
        cp::{Propagator, PropagatorId, Watches},
    },
};

/// A propagator for multiplication (prod = fact1 * fact2) with reification.
///
/// Can handle prod == factn (x = y * x) and fact1 = fact2 (x = y * y)
///
/// Propagations are maximal for active && fact1 != fact2.
/// Explanations are far from minimal.
#[derive(Clone)]
pub(super) struct Mul {
    pub prod: VarRef,
    pub fact1: VarRef,
    pub fact2: VarRef,
    pub active: Lit,
    pub valid: Lit,
}

impl Propagator for Mul {
    fn setup(&self, id: PropagatorId, context: &mut Watches) {
        context.add_watch(self.prod, id);
        context.add_watch(self.fact1, id);
        context.add_watch(self.fact2, id);
        context.add_lit_watch(self.active, id);
        context.add_lit_watch(self.valid, id);
    }

    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        if domains.entails(!self.valid) || domains.entails(!self.active) {
            // constraint is necessarily inactive, no propagations can be made
            return Ok(());
        }

        // If multiplication is trivially inconsistent, we can deactivate the constraint
        if self.trivially_inconsistent(domains) {
            let changed_something = domains.set(!self.active, cause)?;
            debug_assert!(
                changed_something,
                "inconsistent constraint resulted neither in conflict nor in deactivation"
            );
            return Ok(());
        }

        if domains.entails(self.active) && domains.entails(self.valid) {
            // Handle xyx case separately
            if self.xyx_fact().is_some() {
                self.propagate_xyx(domains, cause)?;
            } else {
                // While changes have been made, continue propagating
                while self.propagate_iteration(domains, cause)? {}
            };
        }
        Ok(())
    }

    fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation) {
        // Unfortunately it is very difficult to give minimal explanations due to the iterative nature of the propagation
        // For instance if explanation on product bound is demanded,
        // we would expect it to be the two factor bounds that were multiplied to give that result
        // However, it could be that the factors were updated based on the previous bounds of the product
        // and one of the product bounds would be needed for the explanation

        if literal == !self.active {
            // We must explain a contradiction in the multiplication
            // Just push all variables
            state.explain_var(self.prod, out_explanation);
            state.explain_var(self.fact1, out_explanation);
            state.explain_var(self.fact2, out_explanation);
            return;
        }

        // explanation is always conditioned by the activity of the propagator
        if self.active != Lit::TRUE {
            out_explanation.push(self.active);
            out_explanation.push(self.valid);
        }
        if literal.variable() == self.prod {
            state.explain_var(self.fact1, out_explanation);
            if !self.is_square() {
                state.explain_var(self.fact2, out_explanation);
            }
        } else {
            // Both factors are responsible due to explain_signs function (see PR)
            state.explain_var(self.prod, out_explanation);
            state.explain_var(self.fact1, out_explanation);
            state.explain_var(self.fact2, out_explanation);
        }
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

impl Mul {
    /// Does one iteration of forward and backward propagation, return true if bounds updated
    fn propagate_iteration(&self, domains: &mut Domains, cause: Cause) -> Result<bool, Contradiction> {
        let mut updated = self.propagate_forward(domains, cause)?;
        updated |= self.propagate_signs(domains, cause)?;
        updated |= self.propagate_backward(domains, cause, self.fact1, self.fact2)?;
        updated |= self.propagate_backward(domains, cause, self.fact2, self.fact1)?;
        Ok(updated)
    }

    /// Propagates bounds on product, returns true if bounds updated
    fn propagate_forward(&self, domains: &mut Domains, cause: Cause) -> Result<bool, Contradiction> {
        // Product bounds are max/min of all combinations of factor bounds
        let f1_dom = domains.concrete_domain(self.fact1);
        let f2_dom = domains.concrete_domain(self.fact2);
        let prod = f1_dom * f2_dom;

        domains.set_bounds(self.prod, (prod.lb, prod.ub), cause)
    }

    /// Propagates bounds on fact, return true if bounds updated
    fn propagate_backward(
        &self,
        domains: &mut Domains,
        cause: Cause,
        fact: VarRef,
        other_fact: VarRef,
    ) -> Result<bool, Contradiction> {
        let p = domains.concrete_domain(self.prod);
        let of = domains.concrete_domain(other_fact);
        if p.contains(0) && of.contains(0) {
            // Both upper and lower bounds of fact can be anything since other_fact can be 0
            Ok(false)
        } else if of.is_bound_to(0) {
            Ok(domains.set_bounds(self.prod, (0, 0), cause)?)
        } else if of.lb <= -1 && of.ub >= 1 {
            // Other fact can be 1 or -1, so fact can be as high or low as abs(prod)
            let abs_max = p.lb.abs().max(p.ub.abs());
            Ok(domains.set_bounds(fact, (-abs_max, abs_max), cause)?)
        } else {
            // Case 4a: prod stricly positive or negative, other_fact >= 0
            let mut updated_of = false;
            if !p.contains(0) && of.lb == 0 {
                updated_of |= domains.set_lb(other_fact, 1, cause)?;
            }
            if !p.contains(0) && of.ub == 0 {
                updated_of |= domains.set_ub(other_fact, -1, cause)?;
            }

            // Logic from choco solver adapted to integer division
            let (a, b, (c, d)) = (p.lb, p.ub, domains.bounds(other_fact));

            let (ac_floor, ac_ceil) = div_floor_ceil(a, c);
            let (ad_floor, ad_ceil) = div_floor_ceil(a, d);
            let (bc_floor, bc_ceil) = div_floor_ceil(b, c);
            let (bd_floor, bd_ceil) = div_floor_ceil(b, d);
            let low = ac_ceil.min(ad_ceil).min(bc_ceil).min(bd_ceil);
            let high = ac_floor.max(ad_floor).max(bc_floor).max(bd_floor);
            Ok(domains.set_bounds(fact, (low, high), cause)? || updated_of)
        }
    }

    fn propagate_signs(&self, domains: &mut Domains, cause: Cause) -> Result<bool, Contradiction> {
        // Handle cases like
        // test_propagation(
        //     (36, 67),
        //     (-8, 8),
        //     (-3, 10),
        //     false,
        //     (36, 67),
        //     (4, 8),
        //     (5, 10),
        // );
        // Where -8 * -3 < 36 => f1 >= 0 && f2 >= 0
        let p = domains.concrete_domain(self.prod);
        let f1 = domains.concrete_domain(self.fact1);
        let f2 = domains.concrete_domain(self.fact2);

        if p.contains(0) || !f1.contains(0) || !f2.contains(0) {
            return Ok(false);
        }

        // TODO: more elegant solution
        if f1.lb * f2.lb < p.lb {
            // Change guaranteed
            domains.set_lb(self.fact1, 1, cause)?;
            domains.set_lb(self.fact2, 1, cause)?;
        } else if f1.lb * f2.ub > p.ub {
            domains.set_lb(self.fact1, 1, cause)?;
            domains.set_ub(self.fact2, -1, cause)?;
        } else if f1.ub * f2.lb > p.ub {
            domains.set_lb(self.fact2, 1, cause)?;
            domains.set_ub(self.fact1, -1, cause)?;
        } else if f1.ub * f2.ub < p.lb {
            domains.set_ub(self.fact1, -1, cause)?;
            domains.set_ub(self.fact2, -1, cause)?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    /// Simple propagation for x = y * x
    fn propagate_xyx(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        // Forward propagation
        // Case 1: y spans 1 => x can be anything
        // Case 2: y doesn't span 1 => prod is 0
        let fact = self.xyx_fact().unwrap();
        let prod_dom = domains.concrete_domain(self.prod);
        let fact_dom = domains.concrete_domain(fact);
        if !fact_dom.contains(1) {
            domains.set_bounds(self.prod, (0, 0), cause)?;
        }

        // Backward propagation
        // Case 1: x spans 0 => y can be anything
        // Case 2: x doesn't span 0 => y can only be 1
        if !prod_dom.contains(0) {
            domains.set_bounds(fact, (1, 1), cause)?;
        }

        Ok(())
    }

    /// Check for simple inconsistencies that can quickly be verified without modifying bounds
    fn trivially_inconsistent(&self, domains: &Domains) -> bool {
        let prod = domains.concrete_domain(self.prod);
        let f1 = domains.concrete_domain(self.fact1);
        let f2 = domains.concrete_domain(self.fact2);
        let rhs = f1 * f2;
        prod.disjoint(&rhs)
    }

    /// Returns true if fact1 == fact2
    fn is_square(&self) -> bool {
        self.fact1 == self.fact2
    }

    /// If x = y * x case, returns y
    fn xyx_fact(&self) -> Option<VarRef> {
        if self.fact1 == self.prod {
            Some(self.fact2)
        } else if self.fact2 == self.prod {
            Some(self.fact1)
        } else {
            None
        }
    }
}

// Utils for common operations on domains
impl DomainsSnapshot<'_> {
    /// Creates literal v <= ub(v)
    fn ub_literal(&self, v: VarRef) -> Lit {
        v.leq(self.ub(v))
    }

    /// Creates literal v >= lb(v)
    fn lb_literal(&self, v: VarRef) -> Lit {
        v.geq(self.lb(v))
    }

    // Pushes v <= ub(v) and v >= lb(v) into explanation
    fn explain_var(&self, v: VarRef, out_explanation: &mut Explanation) {
        out_explanation.push(self.lb_literal(v));
        out_explanation.push(self.ub_literal(v));
    }
}

impl Domains {
    // Set upper and lower bounds, return true if either changed
    fn set_bounds(&mut self, v: VarRef, (lb, ub): (IntCst, IntCst), cause: Cause) -> Result<bool, Contradiction> {
        let changed1 = self.set_lb(v, lb, cause)?;
        let changed2 = self.set_ub(v, ub, cause)?;
        Ok(changed1 || changed2)
    }
}

/// Computes div_floor and div_ceil for positive and negative values (using integer division)
fn div_floor_ceil(x: IntCst, y: IntCst) -> (IntCst, IntCst) {
    let quotient_positive = (x >= 0) == (y >= 0);
    let q = x / y;
    let m = x % y;
    (
        q - (m != 0 && !quotient_positive) as IntCst,
        q + (m != 0 && quotient_positive) as IntCst,
    )
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use rand::{Rng, SeedableRng, rngs::SmallRng};

    use super::*;
    use crate::{core::*, reasoners::cp::test::utils::test_explanations};

    // === Assertions ===

    /// Asserts that bounds of var are as expected
    fn check_bounds(v: VarRef, d: &Domains, expected_bounds: (IntCst, IntCst)) {
        assert_eq!(d.bounds(v), expected_bounds, "Unexpected bounds for {v:?}");
    }

    /// Asserts that val is in var's bounds
    fn check_in_bounds(d: &Domains, var: VarRef, val: IntCst) {
        let (lb, ub) = d.bounds(var);
        assert!(lb <= val && val <= ub, "{} <= {} <= {} failed", lb, val, ub);
    }

    /// Asserts that two explanations contain the same literals
    #[allow(unused)]
    fn check_explanations(prop: &Mul, lit: Lit, d: &Domains, expected: Explanation) {
        let out_explanation = &mut Explanation::new();
        prop.explain(lit, &DomainsSnapshot::current(d), out_explanation);
        let expected_set: HashSet<&Lit> = expected.lits.iter().collect();
        let res_set: HashSet<&Lit> = out_explanation.lits.iter().collect();
        assert_eq!(expected_set, res_set);
    }

    // === Utils ===
    #[allow(unused)]
    fn print_domains(d: &Domains, prop: &Mul) {
        println!("Problem: ");
        let (prod_lb, prod_ub) = d.bounds(prop.prod);
        println!("  {prod_lb} <= prod <= {prod_ub}");
        let (fact1_lb, fact1_ub) = d.bounds(prop.fact1);
        println!("  {fact1_lb} <= fact1 <= {fact1_ub}");
        let (fact2_lb, fact2_ub) = d.bounds(prop.fact2);
        println!("  {fact2_lb} <= fact2 <= {fact2_ub}\n");
    }

    /// Generates factors, calculates result, returns propagator and true mult
    fn gen_problems(n: u32, max: u32, always_active: bool) -> Vec<(Domains, Mul, (IntCst, IntCst, IntCst))> {
        let max = max as IntCst;
        let mut res = vec![];
        let mut rng = SmallRng::seed_from_u64(0);
        for _ in 0..n {
            let fact1_val = rng.random_range(-max..max);
            let fact2_val = rng.random_range(-max..max);
            let prod_val = fact1_val * fact2_val;
            let mut d = Domains::new();
            let prod_bounds = (
                rng.random_range(-max * max..=prod_val),
                rng.random_range(prod_val..=max * max),
            );
            let fact1_bounds = (rng.random_range(-max..=fact1_val), rng.random_range(fact1_val..=max));
            let fact2_bounds = (rng.random_range(-max..=fact2_val), rng.random_range(fact2_val..=max));
            let prod = d.new_var(prod_bounds.0, prod_bounds.1);
            let fact1 = d.new_var(fact1_bounds.0, fact1_bounds.1);
            let fact2 = d.new_var(fact2_bounds.0, fact2_bounds.1);
            let prop = Mul {
                prod,
                fact1,
                fact2,
                active: if always_active {
                    Lit::TRUE
                } else {
                    d.new_var(-1, 1).geq(0)
                },
                valid: Lit::TRUE,
            };
            res.push((d, prop, (prod_val, fact1_val, fact2_val)));
        }
        res
    }

    fn gen_square_problems(n: u32, max: u32, always_active: bool) -> Vec<(Domains, Mul, (IntCst, IntCst))> {
        let max = max as IntCst;
        let mut res = vec![];
        let mut rng = SmallRng::seed_from_u64(0);
        for _ in 0..n {
            let fact_val = rng.random_range(-max..max);
            let prod_val = fact_val.pow(2);
            let mut d = Domains::new();
            let prod = d.new_var(
                rng.random_range(-max * max..=prod_val),
                rng.random_range(prod_val..=max * max),
            );
            let fact = d.new_var(rng.random_range(-max..=fact_val), rng.random_range(fact_val..=max));
            let prop = Mul {
                prod,
                fact1: fact,
                fact2: fact,
                active: if always_active {
                    Lit::TRUE
                } else {
                    d.new_var(-1, 1).geq(0)
                },
                valid: Lit::TRUE,
            };
            res.push((d, prop, (prod_val, fact_val)));
        }
        res
    }

    /// Quickly test propagation
    fn test_propagation(
        prod_bounds: (IntCst, IntCst),
        fact1_bounds: (IntCst, IntCst),
        fact2_bounds: (IntCst, IntCst),
        should_fail: bool,
        prop_res: (IntCst, IntCst),
        fact1_res: (IntCst, IntCst),
        fact2_res: (IntCst, IntCst),
    ) {
        let mut d = Domains::new();
        let prop = {
            let d: &mut Domains = &mut d;
            let prod = d.new_var(prod_bounds.0, prod_bounds.1);
            let fact1 = d.new_var(fact1_bounds.0, fact1_bounds.1);
            let fact2 = d.new_var(fact2_bounds.0, fact2_bounds.1);
            Mul {
                prod,
                fact1,
                fact2,
                active: Lit::TRUE,
                valid: Lit::TRUE,
            }
        };

        let res = prop.propagate(&mut d, Cause::Decision);
        assert!(res.is_err() == should_fail, "{:?}", res.err());
        if !should_fail {
            check_bounds(prop.prod, &d, prop_res);
            check_bounds(prop.fact1, &d, fact1_res);
            check_bounds(prop.fact2, &d, fact2_res);
        }
    }

    fn test_square_propagation(
        prod_bounds: (IntCst, IntCst),
        fact_bounds: (IntCst, IntCst),
        should_fail: bool,
        prop_res: (IntCst, IntCst),
        fact_res: (IntCst, IntCst),
    ) {
        let mut d = Domains::new();
        let prod = d.new_var(prod_bounds.0, prod_bounds.1);
        let fact = d.new_var(fact_bounds.0, fact_bounds.1);
        let prop = Mul {
            prod,
            fact1: fact,
            fact2: fact,
            active: Lit::TRUE,
            valid: Lit::TRUE,
        };

        assert!(prop.propagate(&mut d, Cause::Decision).is_err() == should_fail);
        if !should_fail {
            check_bounds(prod, &d, prop_res);
            check_bounds(fact, &d, fact_res);
        }
    }

    fn test_xyx_propagation(
        prod_bounds: (IntCst, IntCst),
        fact_bounds: (IntCst, IntCst),
        should_fail: bool,
        prop_res: (IntCst, IntCst),
        fact_res: (IntCst, IntCst),
    ) {
        let mut d = Domains::new();
        let prod = d.new_var(prod_bounds.0, prod_bounds.1);
        let fact = d.new_var(fact_bounds.0, fact_bounds.1);
        let prop = Mul {
            prod,
            fact1: fact,
            fact2: prod,
            active: Lit::TRUE,
            valid: Lit::TRUE,
        };

        assert!(prop.propagate(&mut d, Cause::Decision).is_err() == should_fail);
        if !should_fail {
            check_bounds(prod, &d, prop_res);
            check_bounds(fact, &d, fact_res);
        }
    }

    fn test_xyx_explanation(prod_bounds: (IntCst, IntCst), fact_bounds: (IntCst, IntCst)) {
        let mut d = Domains::new();
        let prod = d.new_var(prod_bounds.0, prod_bounds.1);
        let fact = d.new_var(fact_bounds.0, fact_bounds.1);
        let prop = Mul {
            prod,
            fact1: fact,
            fact2: prod,
            active: d.new_var(-1, 1).geq(0),
            valid: Lit::TRUE,
        };
        test_explanations(&d, &prop, false);
    }

    // === Tests ===

    #[rustfmt::skip]
    #[test]
    fn test_propagations() {
        // Simple case
        test_propagation(
            (1, 9),
            (2, 3),
            (4, 5),
            false,
            (8, 8),
            (2, 2),
            (4, 4),
        );
        // All 0s
        test_propagation(
            (0, 0),
            (0, 0),
            (0, 0),
            false,
            (0, 0),
            (0, 0),
            (0, 0),
        );
        // -3 * -8 < 36 => f1 >= 0 && f2 >= 0
        test_propagation(
            (36, 67),
            (-8, 8),
            (-3, 10),
            false,
            (36, 67),
            (4, 8),
            (5, 10),
        );
        // Negative form of above
        test_propagation(
            (36, 67),
            (-8, 8),
            (-10, 3),
            false,
            (36, 67),
            (-8, -4),
            (-10, -5),
        );
        // Other negative form of above
        test_propagation(
            (-67, -36),
            (-8, 8),
            (-10, 3),
            false,
            (-67, -36),
            (4, 8),
            (-10, -5),
        );
        // Failure
        test_propagation(
            (1, 10),
            (0, 0),
            (10, 10),
            true,
            (0, 0),  // Ignored
            (0, 0),
            (0, 0),
        );
        // Max and Min int stuff
        // Note that our INT_CST_MAX == -INT_CST_MIN (unlike standard two's complement)
        test_propagation(
            (1, INT_CST_MAX),
            (INT_CST_MIN, 0),
            (-1, 1),
            false,
            (1, INT_CST_MAX),
            (INT_CST_MIN, -1),
            (-1, -1),
        );
        // Check case where there is multiplication between two maxes
        test_propagation(
            (INT_CST_MIN, INT_CST_MAX),
            (INT_CST_MIN, INT_CST_MAX),
            (INT_CST_MIN, INT_CST_MAX),
            false,
            (INT_CST_MIN, INT_CST_MAX),
            (INT_CST_MIN, INT_CST_MAX),
            (INT_CST_MIN, INT_CST_MAX),
        );
    }

    #[rustfmt::skip]
    #[test]
    fn test_square_propagations() {
        // Props aren't minimal, but test a couple cases anyway
        test_square_propagation(
            (25, 26),
            (5, 6),
            false,
            (25, 25),
            (5, 5)
        );
        test_square_propagation(
            (24, 24),
            (5, 6),
            true,
            (0, 0),
            (0, 0)
        );
    }

    #[rustfmt::skip]
    #[test]
    fn test_xyx_propagations() {
        test_xyx_propagation(
            (5, 10),
            (-1, 5),
            false,
            (5, 10),
            (1, 1)
        );
        test_xyx_propagation(
            (0, 10),
            (-5, 5),
            false,
            (0, 10),
            (-5, 5)
        );
        test_xyx_propagation(
            (1, 10),
            (2, 5),
            true,
            (0, 0),
            (0, 0)
        );
        test_xyx_propagation(
            (1, 10),
            (-1, 0),
            true,
            (0, 0),
            (0, 0)
        );
    }

    #[test]
    fn test_xyx_explanations() {
        test_xyx_explanation((5, 10), (-1, 5));
        test_xyx_explanation((0, 10), (-5, 5));
    }

    #[test]
    fn test_propagation_random() {
        // Standard
        for (mut d, prop, (prod_val, fact1_val, fact2_val)) in gen_problems(1000, 10, true) {
            // Propagate and check that bounds are consistent with true values
            assert!(
                prop.propagate(&mut d, Cause::Decision).is_ok(),
                "p={prod_val}, f1={fact1_val}, f2={fact2_val} failed"
            );
            check_in_bounds(&d, prop.prod, prod_val);
            check_in_bounds(&d, prop.fact1, fact1_val);
            check_in_bounds(&d, prop.fact2, fact2_val);
        }
        // Square
        for (mut d, prop, (prod_val, fact_val)) in gen_square_problems(1000, 10, true) {
            // Propagate and check that bounds are consistent with true values
            assert!(
                prop.propagate(&mut d, Cause::Decision).is_ok(),
                "p={prod_val}, f={fact_val} failed"
            );
            check_in_bounds(&d, prop.prod, prod_val);
            check_in_bounds(&d, prop.fact1, fact_val);
        }
    }

    #[test]
    fn test_explanations_random() {
        for (d, prop, _) in gen_problems(1000, 10, false) {
            test_explanations(&d, &prop, false);
        }
        for (d, prop, _) in gen_square_problems(1000, 10, false) {
            test_explanations(&d, &prop, false);
        }
    }
}
