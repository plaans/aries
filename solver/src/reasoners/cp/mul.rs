use std::cmp::{max, min};

use crate::{
    core::{
        state::{Explanation, Term},
        Lit, Relation, VarRef,
    },
    model::extensions::AssignmentExt,
    reasoners::Contradiction,
    reif,
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
        state: &crate::core::state::Domains,
        out_explanation: &mut crate::core::state::Explanation,
    ) {
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
                out_explanation.push(self.original.leq(orig_ub));
                out_explanation.push(self.reified.geq(reif_lb));
            }
            if reif_ub < orig_lb {
                out_explanation.push(self.original.geq(orig_lb));
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
    use itertools::Itertools;
    use rand::prelude::SmallRng;
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng};

    use crate::backtrack::Backtrack;
    use crate::core::literals::Disjunction;
    use crate::core::state::{Event, Origin};
    use crate::core::{IntCst, Relation};
    use crate::{
        core::state::{Cause, Domains, Explainer, Explanation, InferenceCause, InvalidUpdate},
        reasoners::{Contradiction, ReasonerId},
    };

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

    static INFERENCE_CAUSE: Cause = Cause::Inference(InferenceCause {
        writer: ReasonerId::Cp,
        payload: 0,
    });

    /// Test that triggers propagation of random decisions and checks that the explanations are minimal
    #[test]
    fn test_explanations() {
        let mut rng = SmallRng::seed_from_u64(0);
        // function that returns a given number of decisions to be applied later
        // it use the RNG above to drive its random choices
        let mut pick_decisions = |d: &Domains, min: usize, max: usize| -> Vec<Lit> {
            let num_decisions = rng.gen_range(min..=max);
            let vars = d.variables().filter(|v| !d.is_bound(*v)).collect_vec();
            let mut lits = Vec::with_capacity(num_decisions);
            for _ in 0..num_decisions {
                let var_id = rng.gen_range(0..vars.len());
                let var = vars[var_id];
                let (lb, ub) = d.bounds(var);
                let below: bool = rng.gen();
                let lit = if below {
                    let ub = rng.gen_range(lb..ub);
                    Lit::leq(var, ub)
                } else {
                    let lb = rng.gen_range((lb + 1)..=ub);
                    Lit::geq(var, lb)
                };
                lits.push(lit);
            }
            lits
        };
        // new rng for local use
        let mut rng = SmallRng::seed_from_u64(0);

        // repeat a large number of random tests
        for _ in 0..1000 {
            // create the constraint
            let mut d = Domains::new();
            let p = d.new_presence_literal(Lit::TRUE);
            let lit = d.new_presence_literal(p);
            let original = d.new_optional_var(-10, 10, p);
            let reified = d.new_var(-10, 10);
            let mut c = VarEqVarMulLit { reified, original, lit };
            println!("\nConstraint: {c:?} with prez(original) = {p:?}");

            // pick a random set of decisions
            let decisions = pick_decisions(&d, 1, 10);
            println!("Decisions: {decisions:?}");

            // get a copy of the domain on which to apply all decisions
            let mut d = d.clone();
            d.save_state();

            // apply all decisions
            for dec in decisions {
                d.set(dec, Cause::Decision);
            }

            // propagate
            match c.propagate(&mut d, INFERENCE_CAUSE) {
                Ok(()) => {
                    // propagation successful, check that all inferences have correct explanations
                    check_events(&d, &mut c);
                }
                Err(contradiction) => {
                    // propagation failure, check that the contradiction is a valid one
                    let explanation = match contradiction {
                        Contradiction::InvalidUpdate(InvalidUpdate(lit, cause)) => {
                            let mut expl = Explanation::with_capacity(16);
                            expl.push(!lit);
                            d.add_implying_literals_to_explanation(lit, cause, &mut expl, &mut c);
                            expl
                        }
                        Contradiction::Explanation(expl) => expl,
                    };
                    let mut d = d.clone();
                    d.reset();
                    // get the conjunction and shuffle it
                    // note that we do not check minimality here
                    let mut conjuncts = explanation.lits;
                    conjuncts.shuffle(&mut rng);
                    for &conjunct in &conjuncts {
                        let ret = d.set(conjunct, Cause::Decision);
                        if ret.is_err() {
                            println!("Invalid update: {conjunct:?}");
                        }
                    }

                    assert!(
                        c.propagate(&mut d, INFERENCE_CAUSE).is_err(),
                        "Explanation: {conjuncts:?}\n {c:?}"
                    );
                }
            }
        }
    }

    /// Check that all events since the last decision have a minimal explanation
    pub fn check_events(d: &Domains, explainer: &mut (impl Propagator + Explainer)) {
        let events = d
            .trail()
            .events()
            .iter()
            .rev()
            .take_while(|ev| ev.cause != Origin::DECISION)
            .cloned()
            .collect_vec();
        // check that all events have minimal explanations
        for ev in &events {
            check_event_explanation(d, ev, explainer);
        }
    }

    /// Checks that the event has a minimal explanation
    pub fn check_event_explanation(d: &Domains, ev: &Event, explainer: &mut (impl Propagator + Explainer)) {
        let implied = ev.new_literal();
        // generate explanation
        let implicants = d.implying_literals(implied, explainer).unwrap();
        let clause = Disjunction::new(implicants.iter().map(|l| !*l).collect_vec());
        // check minimality
        check_explanation_minimality(d, implied, clause, explainer);
    }

    pub fn check_explanation_minimality(
        domains: &Domains,
        implied: Lit,
        clause: Disjunction,
        propagator: &dyn Propagator,
    ) {
        let mut domains = domains.clone();
        // println!("=== original trail ===");
        // domains.trail().print();
        domains.reset();
        assert!(!domains.entails(implied));

        // gather all decisions not already entailed at root level
        let mut decisions = clause
            .literals()
            .iter()
            .copied()
            .filter(|&l| !domains.entails(l))
            .map(|l| !l)
            .collect_vec();

        for _rotation_id in 0..decisions.len() {
            // println!("Clause: {implied:?} <- {decisions:?}");
            for i in 0..decisions.len() {
                let l = decisions[i];
                if domains.entails(l) {
                    continue;
                }
                // println!("Decide {l:?}");
                domains.decide(l);
                propagator
                    .propagate(&mut domains, INFERENCE_CAUSE)
                    .expect("failed prop");

                let decisions_left = decisions[i + 1..]
                    .iter()
                    .filter(|&l| !domains.entails(*l))
                    .collect_vec();

                if !decisions_left.is_empty() {
                    assert!(!domains.entails(implied), "Not minimal, useless: {:?}", &decisions_left)
                }
            }

            // println!("=== Post trail ===");
            // solver.trail().print();
            assert!(
                domains.entails(implied),
                "Literal {implied:?} not implied after all implicants ({decisions:?}) enforced"
            );
            decisions.rotate_left(1);
        }
    }
}
