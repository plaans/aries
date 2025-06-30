// ========== Constraint ===========

use crate::core::state::*;
use crate::core::*;
use crate::create_ref_type;
use crate::reasoners::Contradiction;

use super::Watches;

/// Unique ID of a propagator (assigned by the CP reasoner)
create_ref_type!(PropagatorId);

/// The propagator trait describe the required implementations for implementing a custom propagator in the CP reasoner.
///
/// # Example
///
/// An example propagator is provided in the `propagator::test::implies` in the same file as the trait definition.
pub trait Propagator: Send {
    /// Set up the watches of the propagator, where `id` is the propagator id that should be placed on the watches.
    /// The propagator is responsible for placing a watch on every bound whose change might require a propagation.
    fn setup(&self, id: PropagatorId, context: &mut Watches);

    /// Perform a full-propagation of the constraint.
    ///
    /// Each change in the domains should be annotated with the given `cause` which acts as a signature to determine
    /// that `self` is the propagator that made the inference and should be called for explaining it.
    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction>;

    /// Explain a previous inference made by the constraint.
    ///
    /// The objective is to determine a set of literals `l1, ..., ln` such that `(l1 & l2 & ... & ln) implies `literal`.
    /// This literals should be appended to the provided `out_explanation`.
    ///
    /// The `state` parameter provides a view of the `domains` as they were at the time the inference was made.
    ///
    /// Important: `literal` may not be exactly the literal inferred but a weaker one. For instance, if the propagation
    /// inferred `(x <= 6)`, the propagator may be asked to explain the literal `(x <= 7)`.
    ///
    /// Note: Though not needed for correctness, it is in general important to have *minimal* explanation (the smallest possible set of implying literals).
    fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation);

    /// Create a boxed version of the propagator.
    fn clone_box(&self) -> Box<dyn Propagator>;
}

impl<T: Propagator> Explainer for T {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &DomainsSnapshot, explanation: &mut Explanation) {
        Propagator::explain(self, literal, model, explanation)
    }
}

/// A simple wrapper around a propagator for dynamic-dipsatch
pub struct DynPropagator {
    pub(super) constraint: Box<dyn Propagator>,
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

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::core::*;

    mod implies {
        //! Example propagator for an implication

        use crate::core::state::*;
        use crate::core::*;
        use crate::reasoners::cp::propagator::*;

        /// An example propagator for an implication constraint (a => b)
        ///
        /// We should propagate (infer) :
        /// - `b` when `a` is true    (case 1)
        /// - `!a` when `b` is false  (case 2)
        #[derive(Clone, Debug)]
        pub struct ImpliesProp {
            pub a: Lit,
            pub b: Lit,
        }

        impl Propagator for ImpliesProp {
            fn setup(&self, id: PropagatorId, context: &mut Watches) {
                // request to be notified whenever `a` of `!b` becomes true
                context.add_lit_watch(self.a, id);
                context.add_lit_watch(!self.b, id);
            }

            fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
                if domains.entails(self.a) {
                    // a is true, we should propagate b
                    // we set `b` to true in the domain which wuold return one of:
                    //  - Ok(true): the change was performed sucessfully
                    //  - Ok(false): nothing was done (i.e. b was already true)
                    //  - Err(xx): contradiction (i.e. b was already false!)
                    //
                    // In the first two cases, we proceed.
                    // If an error was returned, the `?` operator will short-circuit and
                    // immediately return with an appropriate `Contradiction`
                    domains.set(self.b, cause)?;
                }
                // we did not reach an error, propagate the other case
                if domains.entails(!self.b) {
                    domains.set(!self.a, cause)?;
                }
                // if we reach this point, propagation was successful, return Ok
                Ok(())
            }

            fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation) {
                // we are asked to explain a propagation that we previously made

                if self.b.entails(literal) && state.entails(self.a) {
                    // b is stronger that `literal`, meaning setting `b` would also have set `literal`
                    out_explanation.push(self.a);
                } else if (!self.a).entails(literal) && state.entails(!self.b) {
                    out_explanation.push(!self.b);
                } else {
                    panic!("Error: we were asked to explain something we could not have inferred")
                }
            }

            fn clone_box(&self) -> Box<dyn Propagator> {
                Box::new(self.clone())
            }
        }

        //  ===== Tests ======

        use itertools::Itertools;
        use rand::rngs::SmallRng;
        use rand::seq::SliceRandom;
        use rand::{Rng, SeedableRng};

        /// Generates `n` random problems, each with a domain with a few variables and a propagator
        fn implies_problems(n: usize) -> Vec<(Domains, ImpliesProp)> {
            let mut rng = SmallRng::seed_from_u64(0);
            let mut problems = Vec::new();

            for _ in 0..n {
                let mut d = Domains::new();
                let num_vars = rng.gen_range(2..=10);
                let vars = (0..num_vars).map(|_| d.new_var(0, 10)).collect_vec();
                let a = vars.choose(&mut rng).unwrap().leq(rng.gen_range(0..=10));
                let b = vars.choose(&mut rng).unwrap().leq(rng.gen_range(0..=10));
                let a = if rng.gen_bool(0.5) { a } else { !a };
                let b = if rng.gen_bool(0.5) { b } else { !b };
                problems.push((d, ImpliesProp { a, b }));
            }

            problems
        }

        #[test]
        fn test_explanations() {
            use crate::reasoners::cp::propagator::test::utils::*;
            for (d, s) in implies_problems(1000) {
                println!("\nConstraint: {s:?}");
                test_explanations(&d, &s, true);
            }
        }
    }

    pub mod utils {
        //! Utility funcitons for testing propagators

        use itertools::Itertools;
        use rand::{rngs::SmallRng, seq::SliceRandom, Rng, SeedableRng};

        use crate::{
            backtrack::Backtrack,
            core::{
                literals::Disjunction,
                state::{
                    Cause, Domains, DomainsSnapshot, Event, Explainer, Explanation, InferenceCause, InvalidUpdate,
                    Origin,
                },
                Lit,
            },
            reasoners::{cp::Propagator, Contradiction, ReasonerId},
        };

        struct PropExplainer<'a> {
            prop: &'a dyn Propagator,
        }
        impl<'a> Explainer for PropExplainer<'a> {
            fn explain(
                &mut self,
                cause: InferenceCause,
                literal: Lit,
                model: &DomainsSnapshot,
                explanation: &mut Explanation,
            ) {
                self.prop.explain(literal, model, explanation);
            }
        }
        /// Adapts an propagator into an explainer
        /// (the main reason this is need is due to difference in mutability of the explain method of both)
        fn explainer<'a>(prop: &'a dyn Propagator) -> PropExplainer<'a> {
            PropExplainer { prop }
        }
        static INFERENCE_CAUSE: Cause = Cause::Inference(InferenceCause {
            writer: ReasonerId::Cp,
            payload: 0,
        });

        /// Test that triggers propagation of random decisions and checks the explanations are correct
        ///
        /// The test will verify that the explanations:
        ///  - can be generated for all inferences made
        ///  - are correct (calling the propagator with only the implying literals will result in the same inference)
        ///  - are minimal (the inference is not made if any implying literal is missing).
        ///    The minimality check can be deactivated by setting the `check_minimality` parameter to `false`
        ///
        /// IMPORTANT: These tests rely on the `propagate` implementation and are not meaningful if this one is buggy
        /// (but they may show that it is in fact incoherent when called in different contexts)
        pub fn test_explanations(d: &Domains, propagator: &dyn Propagator, check_minimality: bool) {
            use crate::reasoners::cp::propagator::test::utils::pick_decisions;

            let mut decisions_rng = SmallRng::seed_from_u64(0);
            // function that returns a given number of decisions to be applied later
            // it use the RNG above to drive its random choices
            // new rng for local use
            let mut rng = SmallRng::seed_from_u64(0);

            // repeat a large number of random tests
            for _ in 0..100 {
                if d.variables().all(|v| d.is_bound(v)) {
                    println!("Warning: all variables are bound, no tests run");
                    return;
                }

                // pick a random set of decisions
                let decisions = pick_decisions(d, 1, 10, &mut decisions_rng);
                // println!("decisions: {decisions:?}");

                // get a copy of the domain on which to apply all decisions
                let mut d = d.clone();
                d.save_state();

                // apply all decisions (note: some may be ignored because they are no-op or contradictions)
                // println!("Decisions: ");
                for dec in decisions {
                    let res = d.set(dec, Cause::Decision);
                    if res == Ok(true) {
                        // println!("  {dec:?}");
                    }
                }

                // propagate
                match propagator.propagate(&mut d, INFERENCE_CAUSE) {
                    Ok(()) => {
                        // propagation successful, check that all inferences have correct explanations
                        check_events(&d, propagator, check_minimality);
                    }
                    Err(contradiction) => {
                        // propagation failure, check that the contradiction is a valid one
                        let explanation = match contradiction {
                            Contradiction::InvalidUpdate(InvalidUpdate(lit, cause)) => {
                                let mut expl = Explanation::with_capacity(16);
                                expl.push(!lit);
                                let mut explainer = explainer(propagator);
                                d.add_implying_literals_to_explanation(lit, cause, &mut expl, &mut explainer);
                                expl
                            }
                            Contradiction::Explanation(expl) => expl,
                        };
                        let mut d = d.clone();
                        d.reset();
                        // get the conjunction and shuffle it
                        //note that we do not check minimality here
                        let mut conjuncts = explanation.lits;
                        conjuncts.shuffle(&mut rng);
                        for &conjunct in &conjuncts {
                            d.set(conjunct, Cause::Decision);
                        }

                        assert!(
                            propagator.propagate(&mut d, INFERENCE_CAUSE).is_err(),
                            "explanation: {conjuncts:?} did not trigger an inconsistency\n"
                        );
                    }
                }
            }
        }

        /// Utility function to select a number of decisions to be made on a given domain
        ///
        /// Note: some decisions may be redundant or contradictory
        fn pick_decisions(d: &Domains, min: usize, max: usize, rng: &mut SmallRng) -> Vec<Lit> {
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
        }

        /// Check that all events since the last decision have a minimal explanation
        //pub fn check_events(s: &Domains, explainer: &mut (impl Propagator + Explainer)) {
        pub fn check_events(s: &Domains, explainer: &dyn Propagator, check_minimality: bool) {
            let events = s
                .trail()
                .events()
                .iter()
                .rev()
                .take_while(|ev| ev.cause != Origin::DECISION)
                .cloned()
                .collect_vec();
            // check that all events have minimal explanations
            for ev in &events {
                check_event_explanation(s, ev, explainer, check_minimality);
            }
        }

        /// Checks that the event has a minimal explanion
        pub fn check_event_explanation(s: &Domains, ev: &Event, prop: &dyn Propagator, check_minimality: bool) {
            let mut explainer = explainer(prop);
            let implied = ev.new_literal();
            // generate explanation
            let implicants = s.implying_literals(implied, &mut explainer).unwrap();
            let clause = Disjunction::new(implicants.iter().map(|l| !*l).collect_vec());
            // check minimality
            check_explanation_minimality(s, implied, clause, prop, check_minimality);
        }

        pub fn check_explanation_minimality(
            domains: &Domains,
            implied: Lit,
            clause: Disjunction,
            propagator: &dyn Propagator,
            check_minimality: bool,
        ) {
            let mut domains = domains.clone();
            // println!("=== original trail ===");
            // solver.model.domains().trail().print();
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

            // make sure we have at least one propagation
            // TODO: possibly move into loop
            propagator
                .propagate(&mut domains, INFERENCE_CAUSE)
                .expect("failed prop");

            // save the current domains state
            let mut domains_save = domains.clone();

            for _rotation_id in 0..decisions.len() {
                let mut domains = domains_save.clone();
                // println!("Clause: {implied:?} <- {decisions:?}\n");
                for i in 0..decisions.len() {
                    let l = decisions[i];
                    if domains.entails(l) {
                        continue;
                    }
                    // println!("  Decide {l:?}");
                    domains.decide(l);
                    propagator
                        .propagate(&mut domains, INFERENCE_CAUSE)
                        .expect("failed prop");

                    let decisions_left = decisions[i + 1..]
                        .iter()
                        .filter(|&l| !domains.entails(*l))
                        .collect_vec();

                    if !decisions_left.is_empty() && check_minimality {
                        assert!(
                            !domains.entails(implied),
                            "Not minimal, useless: {:?} in implication of {implied:?}",
                            &decisions_left
                        )
                    }
                }

                assert!(
                    domains.entails(implied),
                    "Literal `{implied:?}` not implied after all implicants enforced ({decisions:?})"
                );
                decisions.rotate_left(1);
            }
        }
    }
}
