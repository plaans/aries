//! Utility functions for testing propagators

use itertools::Itertools;
use rand::{Rng, SeedableRng, rngs::SmallRng, seq::SliceRandom};

use crate::{
    backtrack::Backtrack,
    core::{
        Lit,
        literals::Disjunction,
        state::{
            Cause, Domains, DomainsSnapshot, Event, Explainer, Explanation, InferenceCause, InvalidUpdate, Origin,
        },
    },
    reasoners::{Contradiction, ReasonerId, cp::Propagator},
};

struct PropExplainer<'a> {
    prop: &'a dyn Propagator,
}
impl<'a> Explainer for PropExplainer<'a> {
    fn explain(
        &mut self,
        _cause: InferenceCause,
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
pub fn test_explanations(d: &Domains, propagator: &mut dyn Propagator, check_minimality: bool) {
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
                    d.set(conjunct, Cause::Decision).unwrap();
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
    let num_decisions = rng.random_range(min..=max);
    let vars = d.variables().filter(|v| !d.is_bound(*v)).collect_vec();
    let mut lits = Vec::with_capacity(num_decisions);
    for _ in 0..num_decisions {
        let var_id = rng.random_range(0..vars.len());
        let var = vars[var_id];
        let (lb, ub) = d.bounds(var);
        let below: bool = rng.random();
        let lit = if below {
            let ub = rng.random_range(lb..ub);
            Lit::leq(var, ub)
        } else {
            let lb = rng.random_range((lb + 1)..=ub);
            Lit::geq(var, lb)
        };
        lits.push(lit);
    }
    lits
}

/// Check that all events since the last decision have a minimal explanation
pub fn check_events(s: &Domains, explainer: &mut dyn Propagator, check_minimality: bool) {
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
pub fn check_event_explanation(s: &Domains, ev: &Event, prop: &mut dyn Propagator, check_minimality: bool) {
    let mut explainer = explainer(prop);
    let implied = ev.new_literal();
    // generate explanation
    let implicants = s.implying_literals(implied, &mut explainer).unwrap();
    let clause = Disjunction::new(implicants.iter().map(|l| !*l).collect());
    // check minimality
    check_explanation_minimality(s, implied, clause, prop, check_minimality);
}

pub fn check_explanation_minimality(
    domains: &Domains,
    implied: Lit,
    clause: Disjunction,
    propagator: &mut dyn Propagator,
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
    let domains_save = domains.clone();

    for _rotation_id in 0..decisions.len() {
        let mut domains = domains_save.clone();
        // println!("Clause: {implied:?} <- {decisions:?}\n");
        for i in 0..decisions.len() {
            let l = decisions[i];
            if domains.entails(l) {
                continue;
            }
            // println!("  Decide {l:?}");
            domains.decide(l).unwrap();
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
