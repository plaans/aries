// =========== Sum ===========

use crate::backtrack::{DecLvl, EventIndex};
use crate::core::state::{Cause, Domains, DomainsSnapshot, Event, Explanation, InvalidUpdate};
use crate::core::{
    cst_int_to_long, cst_long_to_int, IntCst, Lit, LongCst, SignedVar, VarRef, INT_CST_MAX, INT_CST_MIN,
};
use crate::reasoners::cp::{Propagator, PropagatorId, Watches};
use crate::reasoners::Contradiction;
use itertools::Itertools;
use num_integer::{div_ceil, div_floor, Integer};
use std::cmp::{Ordering, PartialEq};
use std::collections::BinaryHeap;
use std::fmt::{Debug, Formatter};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct SumElem {
    factor: IntCst,
    var: SignedVar,
}

impl std::fmt::Display for SumElem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_assert!(self.factor >= 0);
        write!(f, "{:?}", self.var)?;
        if self.factor != 1 {
            write!(f, " * {}", self.factor)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for SumElem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl SumElem {
    pub fn new(factor: IntCst, var: VarRef) -> Self {
        debug_assert_ne!(factor, 0);
        if factor > 0 {
            Self {
                factor,
                var: SignedVar::plus(var),
            }
        } else {
            Self {
                factor: -factor,
                var: SignedVar::minus(var),
            }
        }
    }

    fn is_constant(&self) -> bool {
        debug_assert!(self.var.variable() != VarRef::ONE); // TODO remove
        false
    }

    fn get_lower_bound(&self, domains: &Domains) -> LongCst {
        debug_assert!(self.factor > 0);
        cst_int_to_long(domains.lb(self.var)).saturating_mul(cst_int_to_long(self.factor))
    }
    fn get_upper_bound(&self, domains: &Domains) -> LongCst {
        debug_assert!(self.factor > 0);
        cst_int_to_long(domains.ub(self.var)).saturating_mul(cst_int_to_long(self.factor))
    }
    fn set_ub(&self, ub: LongCst, domains: &mut Domains, cause: Cause) -> Result<bool, InvalidUpdate> {
        debug_assert!(self.factor > 0);
        let var = self.var;

        // We need to enforce `ub >= var * factor`  with factor > 0
        // enforce  ub / factor >= var
        // equiv to floor(ub / factor) >= var
        let ub = div_floor(ub, cst_int_to_long(self.factor));
        let ub = cst_long_to_int(ub.clamp(cst_int_to_long(INT_CST_MIN), cst_int_to_long(INT_CST_MAX)));
        domains.set_ub(self.var, ub, cause)
    }
}

struct LbBoundEvent<'a> {
    elem: &'a SumElem,
    event: EventIndex,
    domains: &'a DomainsSnapshot<'a>,
}

impl<'a> Debug for LbBoundEvent<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} : {} <- {}", self.elem, self.lb(), self.previous_lb())
    }
}

impl<'a> LbBoundEvent<'a> {
    fn new(elem: &'a SumElem, domains: &'a DomainsSnapshot) -> Option<Self> {
        let var_ub = domains.lb(elem.var);
        let lit = Lit::geq(elem.var, var_ub);
        let event = domains.implying_event(lit)?;

        Some(Self { elem, event, domains })
    }
    fn event(&self) -> &Event {
        self.domains.get_event(self.event)
    }

    fn literal(&self) -> Lit {
        self.event().new_literal()
    }

    /// Lower bound of the element (accounting for the factor) entailed by this event.
    fn lb(&self) -> LongCst {
        // since we are looking for a lower bound, the event will be on an upper bound of the negated variable
        debug_assert_eq!(self.elem.var, -self.event().affected_bound);
        let var_lb = -cst_int_to_long(self.event().new_upper_bound);
        var_lb.saturating_mul(cst_int_to_long(self.elem.factor))
    }

    /// Lower bound of the element (accounting for the factor) BEFORE this event.
    fn previous_lb(&self) -> LongCst {
        // since we are looking for a lower bound, the event will be on an upper bound of the negated variable
        debug_assert_eq!(self.elem.var, -self.event().affected_bound);
        let previous_var_lb = -cst_int_to_long(self.event().previous.upper_bound);
        previous_var_lb.saturating_mul(cst_int_to_long(self.elem.factor))
    }

    /// Returns the previous lower bound event (that preceded this one).
    /// Return `None` if there was no previous event (i.e. the `prev_lb` was entailed at ROOT).
    fn into_previous(self) -> Option<Self> {
        let index = self.event().previous.cause?;
        let previous = Self {
            elem: self.elem,
            event: index,
            domains: self.domains,
        };
        debug_assert_eq!(previous.lb(), self.previous_lb());
        debug_assert!(self > previous, "previous should have lower priority");
        Some(previous)
    }
}

impl<'a> PartialEq<Self> for LbBoundEvent<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.elem == other.elem && self.event == other.event
    }
}

impl<'a> PartialOrd for LbBoundEvent<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Eq for LbBoundEvent<'a> {}

impl<'a> Ord for LbBoundEvent<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Ordering is base on the event ID only
        // later event is bigger
        self.event.cmp(&other.event)
    }
}

#[derive(Clone, Debug)]
pub(super) struct LinearSumLeq {
    pub elements: Vec<SumElem>,
    pub ub: IntCst,
    pub active: Lit,
    /// True if the constraint is within its validity scope
    /// It MUST be the case that `valid => prez(active)`
    pub valid: Lit,
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
    fn print(&self, domains: &Domains) {
        println!("ub: {}", self.ub);
        for e in &self.elements {
            println!(
                " (?{:?}) {:?} x {:?} : [{}, {}]",
                domains.presence(e.var),
                e.factor,
                e.var,
                e.get_lower_bound(domains),
                e.get_upper_bound(domains)
            )
        }
    }
}

impl Propagator for LinearSumLeq {
    fn setup(&self, id: PropagatorId, context: &mut Watches) {
        context.add_watch(self.active.variable(), id);
        context.add_watch(self.valid.variable(), id);
        for e in &self.elements {
            if !e.is_constant() {
                context.add_lb_watch(e.var, id);
            }
        }
    }

    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        if domains.entails(!self.valid) || domains.entails(!self.active) {
            return Ok(()); // constraint is necessarily inactive
        }
        // constraint is not inactive, check validity
        let sum_lb: LongCst = self.elements.iter().map(|e| e.get_lower_bound(domains)).sum();
        let f = cst_int_to_long(self.ub) - sum_lb;

        if f < 0 {
            // INCONSISTENT, it means that the constraint cannot be active.
            // We set `active` to false. If it was already true, it would result in either
            // - a conflict if it is necessarily present (propagated upward with the `?` operator)
            // - making the variable absent, which should set `valid` to false (structural assumption of the constraint)
            let changed_something = domains.set(!self.active, cause)?;
            debug_assert!(
                changed_something,
                "inconsistent constraint resulted neither in conflict nor in deactivation"
            );
            return Ok(());
        }

        if domains.entails(self.active) && domains.entails(self.valid) {
            // constraint is active, we are allowed to propagate this constraint
            for e in &self.elements {
                let lb = e.get_lower_bound(domains);
                let ub = e.get_upper_bound(domains);
                debug_assert!(lb <= ub);
                if ub - lb > f {
                    let new_ub = f + lb;
                    e.set_ub(new_ub, domains, cause)?;
                }
            }
        }
        Ok(())
    }

    fn explain(&self, literal: Lit, domains: &DomainsSnapshot, out_explanation: &mut Explanation) {
        // println!("\n EXPLAIN {literal:?}");
        // a + b + c <= ub
        // we either explain a contradiction:
        //    lb(a) + lb(b) + lb(c) <= ub
        // or an inference  a <= uba
        //      uba <= ub - lb(b) - lb(c)
        //      lb(b) + lb(c) <= ub - uba

        // gather the potential explainers (LHS) in a set of culprits
        //  SUM_{c in culprits) <= UB
        let mut culprits = BinaryHeap::new();

        let mut ub = cst_int_to_long(self.ub);
        if literal == !self.active {
            // we are explaining a contradiction hence we must show that our lower bounds are strictly greater than the upper bound
            ub += 1;
        } else {
            // we are NOT explaining a contradiction, at least one element be the subject of the explanation
            debug_assert!(self.elements.iter().any(|e| e.var == literal.svar()));
            if self.active != Lit::TRUE {
                // explanation is always conditioned by the activity of the propagator
                out_explanation.push(self.active);
                out_explanation.push(self.valid);
            }
        }
        for e in &self.elements {
            if e.var == literal.svar() {
                let factor = cst_int_to_long(e.factor);
                // this is the element to explain
                // move its upper bound to the RHS
                let a_ub = cst_int_to_long(literal.ub_value()).saturating_mul(factor);
                // the inference is:   factor * e.var <= a_ub
                //  e.var <= a_ub / factor
                // because e.var is integral, we can increase a_ub until its is immediately before the next multiple of factor
                // without changing the result
                let a_ub = div_floor(a_ub, factor) * factor + factor - 1;
                debug_assert!(div_floor(a_ub, factor) <= cst_int_to_long(literal.ub_value()));
                // println!("culprit {e:?}");
                ub -= a_ub;
            } else if let Some(event) = LbBoundEvent::new(e, domains) {
                // there is a lower bound event on this element, add it to the set of culprits for later processing
                culprits.push(event)
            } else {
                // no event associated to the element, which means its value is entailed at the ROOT
                // Hence it does need to be present in the explanation, but should cancel its contribution to the UB
                let elem_var_lb = cst_int_to_long(domains.lb(e.var));
                debug_assert_eq!(
                    domains.entailing_level(Lit::geq(e.var, elem_var_lb as IntCst)),
                    DecLvl::ROOT
                );
                let elem_lb = elem_var_lb.saturating_mul(cst_int_to_long(e.factor));
                // println!("move left: {e:?} >= {elem_lb}");
                ub -= elem_lb;
            }
        }

        let sum_lb = |culps: &BinaryHeap<LbBoundEvent>| -> LongCst { culps.iter().map(|e| e.lb()).sum() };
        let print = |culps: &BinaryHeap<LbBoundEvent>| {
            println!("QUEUE:");
            for e in culps.iter() {
                println!(
                    " {:?} ({:?}) {:?} {:?}    {:?}",
                    e.literal(),
                    &e.elem,
                    e.lb(),
                    e.previous_lb(),
                    e.event
                )
            }
        };
        // print(&culprits);

        let mut culprits_lb = sum_lb(&culprits);
        // println!("BEFORE LOOP: {culprits_lb}   <= {ub}");
        while let Some(elem_event) = culprits.pop() {
            // let e = &elem_event;
            // println!(
            //     " {:?} ({:?}) {:?} {:?}    {:?}",
            //     e.literal(),
            //     &e.elem,
            //     e.lb(),
            //     e.previous_lb(),
            //     e.event
            // );
            let event_idx = elem_event.event;
            let lb = elem_event.lb();
            let prev_lb = elem_event.previous_lb();
            culprits_lb -= lb; // update the
            debug_assert_eq!(culprits_lb, sum_lb(&culprits));

            debug_assert!(ub <= culprits_lb + lb);
            if ub <= culprits_lb + prev_lb {
                // this event is not necessary and considering the previous one would be sufficient for the explanation
                if let Some(previous) = elem_event.into_previous() {
                    // add the previous lower bound for later processing
                    culprits.push(previous);
                    culprits_lb += prev_lb;
                    // println!("  > to prev")
                } else {
                    // there was no previous event (ie the previous lower bound always holds)
                    debug_assert_eq!(
                        domains.entailing_level(domains.get_event(event_idx).previous_literal()),
                        DecLvl::ROOT
                    );
                    // no need to add to the explanation (tautology) but cancel its contribution
                    ub -= prev_lb;
                    // println!("  > folded")
                }
            } else {
                // this event is necessary, add it to the explanation
                out_explanation.push(elem_event.literal());
                ub -= lb;
                // println!("  > select")
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtrack::Backtrack;
    use crate::core::literals::Disjunction;
    use crate::core::state::{Explainer, InferenceCause, Origin};
    use crate::core::SignedVar;
    use crate::reasoners::ReasonerId;
    use rand::prelude::SmallRng;
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng};
    /* ============================== Factories ============================= */

    fn var(lb: IntCst, ub: IntCst, factor: IntCst, dom: &mut Domains) -> SumElem {
        let x = dom.new_var(lb, ub);
        SumElem::new(factor, x)
    }

    fn sum(elements: Vec<SumElem>, ub: IntCst, active: Lit) -> LinearSumLeq {
        LinearSumLeq {
            elements,
            ub,
            active,
            valid: Lit::TRUE,
        }
    }

    /* =============================== Helpers ============================== */

    fn check_bounds(e: &SumElem, d: &Domains, lb: IntCst, ub: IntCst) {
        assert_eq!(e.get_lower_bound(d), lb.into());
        assert_eq!(e.get_upper_bound(d), ub.into());
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
        check_bounds(&v, &d, -200, 200);
        assert_eq!(v.set_ub(50, &mut d, Cause::Decision), Ok(true));
        check_bounds(&v, &d, -200, 50);
        assert_eq!(v.set_ub(50, &mut d, Cause::Decision), Ok(false));
        check_bounds(&v, &d, -200, 50);
    }

    #[test]
    /// Tests that the upper bound of a constant can be changed if it is greater or equal to the current value
    fn test_ub_setter_cst() {
        let mut d = Domains::new();
        let c = var(3, 3, 1, &mut d);
        let s = sum(vec![c], 10, Lit::TRUE);
        check_bounds(&c, &d, 3, 3);
        assert_eq!(c.set_ub(50, &mut d, Cause::Decision), Ok(false));
        check_bounds(&c, &d, 3, 3);
        assert_eq!(c.set_ub(3, &mut d, Cause::Decision), Ok(false));
        check_bounds(&c, &d, 3, 3);
        assert!(c.set_ub(0, &mut d, Cause::Decision).is_err());
        check_bounds(&c, &d, 3, 3);
    }

    #[test]
    /// Tests on the constraint `2*x + 3 <= 10` with `x` in `[-100, 100]`
    fn test_single_var_constraint() {
        let mut d = Domains::new();
        let x = var(-100, 100, 2, &mut d);
        let c = var(3, 3, 1, &mut d);
        let s = sum(vec![x, c], 10, Lit::TRUE);

        // Check bounds
        check_bounds(&x, &d, -200, 200);
        check_bounds(&c, &d, 3, 3);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(&x, &d, -200, 6); // We should have an upper bound of 7 but `x` is an integer so we have `x=floor(7/2)*2`
        check_bounds(&c, &d, 3, 3);

        // Possible ub setting
        assert_eq!(x.set_ub(5, &mut d, Cause::Decision), Ok(true));
        check_bounds(&x, &d, -200, 4);
        check_bounds(&c, &d, 3, 3);

        // Impossible ub setting
        assert_eq!(x.set_ub(10, &mut d, Cause::Decision), Ok(false));
        check_bounds(&x, &d, -200, 4);
        check_bounds(&c, &d, 3, 3);
    }

    #[test]
    /// Tests on the constraint `2*x + 3*y + z + 25 <= 10` with variables in `[-100, 100]`
    fn test_multi_var_constraint() {
        let mut d = Domains::new();
        let x = var(-100, 100, 2, &mut d);
        let y = var(-100, 100, 3, &mut d);
        let z = var(-100, 100, 1, &mut d);
        let c = var(25, 25, 1, &mut d);
        let s = sum(vec![x, y, z, c], 10, Lit::TRUE);

        // Check bounds
        check_bounds(&x, &d, -200, 200);
        check_bounds(&y, &d, -300, 300);
        check_bounds(&z, &d, -100, 100);
        check_bounds(&c, &d, 25, 25);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(&x, &d, -200, 200);
        check_bounds(&y, &d, -300, 285);
        check_bounds(&z, &d, -100, 100);
        check_bounds(&c, &d, 25, 25);
    }

    #[test]
    /// Tests on the constraint `2*x - 3*y + 0*z + 25 <= 10` with variables in `[-100, 100]`
    fn test_neg_factor_constraint() {
        let mut d = Domains::new();
        let x = var(-100, 100, 2, &mut d);
        let y = var(-100, 100, -3, &mut d);
        let z = var(0, 0, 1, &mut d);
        let c = var(25, 25, 1, &mut d);
        let s = sum(vec![x, y, z, c], 10, Lit::TRUE);

        // Check bounds
        check_bounds(&x, &d, -200, 200);
        check_bounds(&y, &d, -300, 300);
        check_bounds(&z, &d, 0, 0);
        check_bounds(&c, &d, 25, 25);

        // Check propagation
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        check_bounds(&x, &d, -200, 200);
        check_bounds(&y, &d, -300, 183);
        check_bounds(&z, &d, 0, 0);
        check_bounds(&c, &d, 25, 25);
    }

    #[test]
    /// Test that the explanation of an impossible sum `25 <= 10` is its present
    fn test_explanation_present_impossible_sum() {
        let mut d = Domains::new();
        let v = d.new_var(-1, 1);
        let c = var(0, 25, 1, &mut d);
        let s = sum(vec![c], 10, v.lt(0));

        // take a decision that will make the constraint unsatifiable and thus deactivate it
        d.save_state();
        d.decide(c.var.geq(25));
        assert!(s.propagate(&mut d, Cause::Decision).is_ok());
        // constraint should have been deactivated
        check_bounds_var(v, &d, 0, 1);

        let mut expl = Explanation::new();
        let d = DomainsSnapshot::current(&d);
        s.explain(v.geq(0), &d, &mut expl);
        assert_eq!(expl.lits, vec![c.var.geq(25)]);
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

        // a set of constraints to be tested individually
        let constraints: &[(&[IntCst], IntCst)] = &[
            (&[2, 4, 6, 4], 30), // 2x1 + 4x2 + 6x3 + 4x4 <= 30
            (&[2, -4, 6, -1, 4, 3], 5),
            (&[2, 4, 6, 4, 5], 30),
            (&[2, 4, 6, 4], 60),
            (&[2, -4, 6, -1, 4, 3], 5),
            (&[2, -1, 6, 4, -3], -17),
        ];

        for (weights, ub) in constraints {
            // we have one constraint to test
            let mut d = Domains::new();
            let vars = (0..weights.len()).map(|i| d.new_var(0, 10)).collect_vec();
            let elems = weights
                .iter()
                .zip(vars.iter())
                .map(|(w, v)| SumElem::new(*w, *v))
                .collect_vec();

            let mut s = sum(elems, *ub, Lit::TRUE);
            println!("\nConstraint: {s:?}");

            // repeat a large number of random tests
            for _ in 0..1000 {
                // pick a random set of decisions
                let decisions = pick_decisions(&d, 1, 10);
                println!("decisions: {decisions:?}");

                // get a copy of the domain on which to apply all decisions
                let mut d = d.clone();
                d.save_state();

                // apply all decisions
                for dec in decisions {
                    d.set(dec, Cause::Decision);
                }

                // propagate
                match s.propagate(&mut d, INFERENCE_CAUSE) {
                    Ok(()) => {
                        // propagation successful, check that all inferences have correct explanations
                        check_events(&d, &mut s);
                    }
                    Err(contradiction) => {
                        // propagation failure, check that the contradiction is a valid one
                        let explanation = match contradiction {
                            Contradiction::InvalidUpdate(InvalidUpdate(lit, cause)) => {
                                let mut expl = Explanation::with_capacity(16);
                                expl.push(!lit);
                                d.add_implying_literals_to_explanation(lit, cause, &mut expl, &mut s);
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
                            s.propagate(&mut d, INFERENCE_CAUSE).is_err(),
                            "explanation: {conjuncts:?}\n {s:?}"
                        );
                    }
                }
            }
        }
    }

    /// Check that all events since the last decision have a minimal explanation
    pub fn check_events(s: &Domains, explainer: &mut (impl Propagator + Explainer)) {
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
            check_event_explanation(s, ev, explainer);
        }
    }

    /// Checks that the event has a minimal explanion
    pub fn check_event_explanation(s: &Domains, ev: &Event, explainer: &mut (impl Propagator + Explainer)) {
        let implied = ev.new_literal();
        // generate explantion
        let implicants = s.implying_literals(implied, explainer).unwrap();
        let clause = Disjunction::new(implicants.iter().map(|l| !*l).collect_vec());
        // check minimality
        check_explanation_minimality(s, implied, clause, explainer);
    }

    pub fn check_explanation_minimality(
        domains: &Domains,
        implied: Lit,
        clause: Disjunction,
        propagator: &dyn Propagator,
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

        for _rotation_id in 0..decisions.len() {
            // println!("\nClause: {implied:?} <- {decisions:?}\n");
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
                "Literal not implied after all implicants enforced"
            );
            decisions.rotate_left(1);
        }
    }
}
