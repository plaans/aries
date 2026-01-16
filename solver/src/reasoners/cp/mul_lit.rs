use std::cmp::{max, min};

use crate::{
    core::{Lit, Relation, VarRef},
    model::extensions::DomainsExt,
};

use super::Propagator;

#[derive(Clone, Debug)]
/// Propagator for the constraint `reified <=> original * lit`
pub(super) struct VarEqVarMulLit {
    pub reified: VarRef,
    pub original: VarRef,
    pub lit: Lit,
}

impl std::fmt::Display for VarEqVarMulLit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} <=> {:?} * {:?}", self.reified, self.original, self.lit)
    }
}

impl Propagator for VarEqVarMulLit {
    fn setup(&self, id: super::PropagatorId, context: &mut super::Watches) {
        context.add_watch(self.reified, id);
        context.add_watch(self.original, id);
        context.add_watch(self.lit.variable(), id);
    }

    fn propagate(
        &self,
        domains: &mut crate::core::state::Domains,
        cause: crate::core::state::Cause,
    ) -> Result<(), crate::reasoners::Contradiction> {
        let n = domains.trail().len();

        let orig_prez = domains.presence_literal(self.original);
        debug_assert!(domains.implies(self.lit, orig_prez));

        if domains.entails(self.lit) {
            // lit is true, so reified = original
            let (orig_lb, orig_ub) = domains.bounds(self.original);
            let (reif_lb, reif_ub) = domains.bounds(self.reified);

            domains.set_lb(self.reified, orig_lb, cause)?;
            domains.set_ub(self.reified, orig_ub, cause)?;
            domains.set_lb(self.original, reif_lb, cause)?;
            domains.set_ub(self.original, reif_ub, cause)?;
        } else if domains.entails(!self.lit) {
            // lit is false, so reified = 0
            domains.set_lb(self.reified, 0, cause)?;
            domains.set_ub(self.reified, 0, cause)?;
        } else {
            // lit is not fixed
            let (orig_lb, orig_ub) = domains.bounds(self.original);
            let (reif_lb, reif_ub) = domains.bounds(self.reified);

            if reif_lb > orig_ub || reif_ub < orig_lb {
                // intersection(dom(reif), dom(orig)) is empty, so lit = false and reified = 0
                domains.set(!self.lit, cause)?;
                domains.set_lb(self.reified, 0, cause)?;
                domains.set_ub(self.reified, 0, cause)?;
            } else if reif_lb > 0 || reif_ub < 0 {
                // reified != 0, so lit = true
                domains.set(self.lit, cause)?;
            } else if domains.entails(orig_prez) {
                // original is present, can reduce the bounds of reified while keeping 0 in the domain
                domains.set_lb(self.reified, min(0, orig_lb), cause)?;
                domains.set_ub(self.reified, max(0, orig_ub), cause)?;
            }
        }

        if n != domains.trail().len() {
            // At least one domain has been modified
            self.propagate(domains, cause)
        } else {
            Ok(())
        }
    }

    fn explain(
        &self,
        literal: Lit,
        state: &crate::core::state::DomainsSnapshot,
        out_explanation: &mut crate::core::state::Explanation,
    ) {
        // At least one element of the constraint must be the subject of the explanation
        debug_assert!(
            [self.reified, self.original, self.lit.variable()]
                .iter()
                .any(|&v| v == literal.variable())
        );

        let (reif_lb, reif_ub) = state.bounds(self.reified);
        let (orig_lb, orig_ub) = state.bounds(self.original);
        let orig_prez = state.presence(self.original);

        if literal == self.lit {
            // Explain why lit is true
            // reified != 0, so lit = true
            if reif_lb > 0 {
                out_explanation.push(self.reified.geq(reif_lb));
            } else if reif_ub < 0 {
                out_explanation.push(self.reified.leq(reif_ub));
            }
        } else if literal == !self.lit {
            // Explain why lit is false
            // intersection(dom(reif), dom(orig)) is empty, so lit = false
            if reif_lb > orig_ub {
                out_explanation.push(self.original.lt(reif_lb));
                out_explanation.push(self.reified.geq(reif_lb));
            }
            if reif_ub < orig_lb {
                out_explanation.push(self.original.gt(reif_ub));
                out_explanation.push(self.reified.leq(reif_ub));
            }
        } else {
            let (var, rel, val) = literal.unpack();
            if var == self.reified {
                // Explain the bounds of reified
                match rel {
                    Relation::Gt => {
                        if val < 0 {
                            if state.entails(!orig_prez) {
                                out_explanation.push(!orig_prez);
                            } else if state.entails(!self.lit) {
                                out_explanation.push(!self.lit);
                            } else if state.entails(orig_prez) {
                                out_explanation.push(orig_prez);
                                out_explanation.push(self.original.gt(val));
                            }
                        } else if state.entails(self.lit) {
                            out_explanation.push(self.lit);
                            out_explanation.push(self.original.gt(val));
                        }
                    }
                    Relation::Leq => {
                        if val >= 0 {
                            if state.entails(!orig_prez) {
                                out_explanation.push(!orig_prez);
                            } else if state.entails(!self.lit) {
                                out_explanation.push(!self.lit);
                            } else if state.entails(orig_prez) {
                                out_explanation.push(orig_prez);
                                out_explanation.push(self.original.leq(val));
                            }
                        } else if state.entails(self.lit) {
                            out_explanation.push(self.lit);
                            out_explanation.push(self.original.leq(val));
                        }
                    }
                };
            } else if var == self.original && state.entails(self.lit) {
                // Explain the bounds of original
                out_explanation.push(self.lit);
                match rel {
                    Relation::Gt => out_explanation.push(self.reified.gt(val)),
                    Relation::Leq => out_explanation.push(self.reified.leq(val)),
                };
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use rand::prelude::SmallRng;
    use rand::{Rng, SeedableRng};

    use crate::core::IntCst;
    use crate::core::state::{Cause, Domains};

    use super::*;

    fn mul(reif: VarRef, orig: VarRef, lit: Lit) -> VarEqVarMulLit {
        VarEqVarMulLit {
            reified: reif,
            original: orig,
            lit,
        }
    }

    fn check_bounds(v: VarRef, d: &Domains, lb: IntCst, ub: IntCst) {
        assert_eq!(d.lb(v), lb);
        assert_eq!(d.ub(v), ub);
    }

    #[test]
    fn test_propagation_with_true_lit() {
        let mut d = Domains::new();
        let r = d.new_var(-5, 10);
        let o = d.new_var(-10, 5);
        let c = mul(r, o, Lit::TRUE);

        // Check initial bounds
        check_bounds(r, &d, -5, 10);
        check_bounds(o, &d, -10, 5);

        // Check propagation
        assert!(c.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(r, &d, -5, 5);
        check_bounds(o, &d, -5, 5);
    }

    #[test]
    fn test_propagation_with_false_lit() {
        let mut d = Domains::new();
        let r = d.new_var(-5, 10);
        let o = d.new_var(-10, 5);
        let c = mul(r, o, Lit::FALSE);

        // Check initial bounds
        check_bounds(r, &d, -5, 10);
        check_bounds(o, &d, -10, 5);

        // Check propagation
        assert!(c.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(r, &d, 0, 0);
        check_bounds(o, &d, -10, 5);
    }

    #[test]
    fn test_propagation_with_non_zero_reif() {
        let mut d = Domains::new();
        let p = d.new_presence_literal(Lit::TRUE);
        let r = d.new_var(1, 10);
        let o = d.new_optional_var(-10, 5, p);
        let l = d.new_presence_literal(p);
        let c = mul(r, o, l);

        // Check initial bounds
        check_bounds(r, &d, 1, 10);
        check_bounds(o, &d, -10, 5);
        check_bounds(l.variable(), &d, 0, 1);

        // Check propagation
        assert!(c.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(r, &d, 1, 5);
        check_bounds(o, &d, 1, 5);
        check_bounds(l.variable(), &d, 1, 1);
    }

    #[test]
    fn test_propagation_with_exclusive_bounds() {
        let mut d = Domains::new();
        let p = d.new_presence_literal(Lit::TRUE);
        let r = d.new_var(-5, 0);
        let o = d.new_optional_var(1, 5, p);
        let l = d.new_presence_literal(p);
        let c = mul(r, o, l);

        // Check initial bounds
        check_bounds(r, &d, -5, 0);
        check_bounds(o, &d, 1, 5);
        check_bounds(l.variable(), &d, 0, 1);

        // Check propagation
        assert!(c.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(r, &d, 0, 0);
        check_bounds(o, &d, 1, 5);
        check_bounds(l.variable(), &d, 0, 0);
    }

    fn gen_problems(n: usize) -> Vec<(Domains, VarEqVarMulLit)> {
        let mut problems = Vec::new();
        let mut rng = SmallRng::seed_from_u64(0);

        // repeat a large number of random tests
        for _ in 0..n {
            let mut d = Domains::new();

            let prez = d.new_presence_literal(Lit::TRUE);
            let lit = d.new_presence_literal(prez);

            let reif_lb = rng.random_range(-20..=20);
            let reif_ub = rng.random_range(-20..=20).max(reif_lb);
            let reified = d.new_var(reif_lb, reif_ub);

            let orig_prez = d.new_presence_literal(prez);
            let orig_lb = rng.random_range(-20..=20);
            let orig_ub = rng.random_range(-20..=20).max(orig_lb);
            let original = d.new_optional_var(orig_lb, orig_ub, orig_prez);
            d.add_implication(lit, orig_prez);

            let c = VarEqVarMulLit { reified, original, lit };
            problems.push((d, c));
        }
        problems
    }

    #[test]
    fn test_explanations() {
        use crate::reasoners::cp::propagator::test::utils::*;
        for (d, c) in gen_problems(100) {
            println!("\nConstraint: {c:?}");
            test_explanations(&d, &c, true);
        }
    }
}
