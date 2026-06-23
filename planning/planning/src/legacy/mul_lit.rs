use aries_solver::{
    core::{
        state::{Cause, DomainsSnapshot, Explanation, OptDomain},
        IntCst,
    },
    lang::{BoolExpr, Store},
    prelude::{Conjunction, Domains, Solution},
    reasoners::{
        cp::{Propagator, PropagatorId, UserPropagator, Watches},
        Contradiction,
    },
};
use std::fmt::*;

#[derive(Clone)]
pub struct EqVarMulLit {
    pub lhs: Var,
    pub rhs: Var,
    pub lit: Lit,
}

impl Debug for EqVarMulLit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.lit, self.rhs)
    }
}

impl EqVarMulLit {
    pub fn new(lhs: impl Into<Var>, rhs: impl Into<Var>, lit: impl Into<Lit>) -> Self {
        let lhs = lhs.into();
        let rhs = rhs.into();
        let lit = lit.into();
        Self { lhs, rhs, lit }
    }
}

// #[derive(Eq, PartialEq, Hash, Clone)]
// pub struct NFEqVarMulLit {
//     pub lhs: Var,
//     pub rhs: Var,
//     pub lit: Lit,
// }

// impl Debug for NFEqVarMulLit {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{:?} = {:?} * {:?}", self.lhs, self.lit, self.rhs)
//     }
// }

impl<Ctx: Store> BoolExpr<Ctx> for EqVarMulLit {
    fn enforce_if(&self, implicant: aries_solver::prelude::Lit, ctx: &mut Ctx) {
        assert!(ctx.entails(implicant));
        let propagator = VarEqVarMulLit {
            reified: self.lhs,
            original: self.rhs,
            lit: self.lit,
        };
        ctx.enforce_user_propagator(propagator);
    }

    fn conj_scope(&self, ctx: &Ctx) -> aries_solver::prelude::Conjunction {
        Conjunction::from(ctx.presence_literal(self.lhs))
    }
}

use std::cmp::{max, min};

use aries_solver::{
    core::{Lit, Relation, Var},
    model::extensions::DomainsExt,
};

#[derive(Clone, Debug)]
/// Propagator for the constraint `reified <=> original * lit`
pub(super) struct VarEqVarMulLit {
    pub reified: Var,
    pub original: Var,
    pub lit: Lit,
}

impl std::fmt::Display for VarEqVarMulLit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} <=> {:?} * {:?}", self.reified, self.original, self.lit)
    }
}

impl Propagator for VarEqVarMulLit {
    fn setup(&self, id: PropagatorId, context: &mut Watches) {
        context.add_watch(self.reified, id);
        context.add_watch(self.original, id);
        context.add_watch(self.lit.variable(), id);
    }

    fn propagate(&mut self, domains: &mut Domains, cause: Cause) -> std::result::Result<(), Contradiction> {
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

    fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation) {
        // At least one element of the constraint must be the subject of the explanation
        debug_assert!([self.reified, self.original, self.lit.variable()]
            .iter()
            .any(|&v| v == literal.variable()));

        let (reif_lb, reif_ub) = state.bounds(self.reified);
        let (orig_lb, orig_ub) = state.bounds(self.original);
        let orig_prez = state.presence_literal(self.original);

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

impl UserPropagator for VarEqVarMulLit {
    fn get_propagators(&self) -> Vec<aries_solver::reasoners::cp::DynPropagator> {
        vec![self.clone().into()]
    }

    fn satisfied(&self, sol: &Solution) -> bool {
        let lhs = &self.reified;
        let rhs = &self.original;
        let lit = &self.lit;
        let prez = |var| sol.present(var).unwrap();
        let value = |var| match sol.opt_domain_of(var) {
            OptDomain::Present(lb, ub) if lb == ub => lb,
            _ => panic!(),
        };
        let lvalue = |lit: Lit| sol.value_of(lit).unwrap();
        if !prez(*lhs) {
            true
        } else if !prez(lit.variable()) {
            if !prez(*rhs) {
                true
            } else {
                value(*lhs) == 0 && value(*rhs) == 0
            }
        } else {
            let lit_value: IntCst = lvalue(*lit).into();
            if !prez(*rhs) {
                value(*lhs) == 0 && lit_value == 0
            } else {
                value(*lhs) == lit_value * value(*rhs)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use aries_solver::reasoners::cp::testing::test_explanations;
    use rand::prelude::SmallRng;
    use rand::{Rng, SeedableRng};

    use aries_solver::core::state::{Cause, Domains};
    use aries_solver::core::IntCst;

    use super::*;

    fn mul(reif: Var, orig: Var, lit: Lit) -> VarEqVarMulLit {
        VarEqVarMulLit {
            reified: reif,
            original: orig,
            lit,
        }
    }

    fn check_bounds(v: Var, d: &Domains, lb: IntCst, ub: IntCst) {
        assert_eq!(d.lb(v), lb);
        assert_eq!(d.ub(v), ub);
    }

    #[test]
    fn test_propagation_with_true_lit() {
        let mut d = Domains::new();
        let r = d.new_var(-5, 10);
        let o = d.new_var(-10, 5);
        let mut c = mul(r, o, Lit::TRUE);

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
        let mut c = mul(r, o, Lit::FALSE);

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
        let mut c = mul(r, o, l);

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
        let mut c = mul(r, o, l);

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
    fn test_explanations_eq_var_mul_lit() {
        for (d, mut c) in gen_problems(100) {
            println!("\nConstraint: {c:?}");
            test_explanations(&d, &mut c, true);
        }
    }
}
