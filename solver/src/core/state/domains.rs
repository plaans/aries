use crate::backtrack::{Backtrack, DecLvl, DecisionLevelClass, EventIndex, ObsTrail};
use crate::collections::ref_store::RefMap;
use crate::core::literals::{Disjunction, DisjunctionBuilder, ImplicationGraph, LitSet};
use crate::core::state::cause::{DirectOrigin, Origin};
use crate::core::state::event::Event;
use crate::core::state::int_domains::IntDomains;
use crate::core::state::{Cause, DomainsSnapshot, Explainer, Explanation, ExplanationQueue, InvalidUpdate, OptDomain};
use crate::core::*;
use std::fmt::{Debug, Formatter};

#[cfg(debug_assertions)]
pub mod witness;

mod minimize;

/// Structure that contains the domains of optional variable.
///
/// Internally an optional variable is split between
///  - a presence literal that is true iff the variable is present
///  - an integer variable that give the domain of the optional variable if is is present.
///
/// Note that under this scheme, a non-optional variable could be represented a variable whose presence literal is
/// the `TRUE` literal.
///
/// Invariant:
///  - all presence variables are non-optional
///  - a presence variable `a` might be declared with a *scope* literal `b`, meaning that `b => a`
///  - every variable always have a valid domain (which might be the empty domain if the variable is optional)
///  - if an update would cause the integer domain of an optional variable to become empty, its presence variable would be set to false
///  - the implication relations between the presence variables and their scope are automatically propagated.
#[derive(Clone)]
pub struct Domains {
    /// Integer part of the domains.
    pub(super) doms: IntDomains,
    /// If a variable is optional, associates it with a literal that
    /// is true if and only if the variable is present.
    presence: RefMap<VarRef, Lit>,
    /// A graph to encode the relations between presence variables.
    implications: ImplicationGraph,
    /// A queue used internally when building explanations. Only useful to avoid repeated allocations.
    queue: ExplanationQueue,
}

impl Domains {
    pub fn new() -> Self {
        let domains = Domains {
            doms: IntDomains::new(),
            presence: Default::default(),
            implications: Default::default(),
            queue: Default::default(),
        };
        debug_assert!(domains.entails(Lit::TRUE));
        debug_assert!(!domains.entails(Lit::FALSE));
        domains
    }

    pub fn new_var(&mut self, lb: IntCst, ub: IntCst) -> VarRef {
        self.doms.new_var(lb, ub)
    }

    /// Records a direct implication `from => to`
    ///
    /// # Assumptions
    ///
    /// - `from` and `to` are non-optional
    /// - Propagating the implication will not create an inconsistencies
    ///
    /// The current implementation will panic in those cases, but this check might be done in
    /// debug-only builds at a later time.
    #[rustfmt::skip]
    pub fn add_implication(&mut self, from: Lit, to: Lit) {
        assert_eq!(self.presence(from.variable()), Lit::TRUE, "Implication only supported between non-optional variables");
        assert_eq!(self.presence(to.variable()), Lit::TRUE, "Implication only supported between non-optional variables");
        self.implications.add_implication(from, to);
        if self.entails(from) {
            let prop_result = self.set_impl(to, DirectOrigin::ImplicationPropagation(from));
            assert!(prop_result.is_ok(), "{}", "Inconsistency on the addition of implies({from:?}, {to:?}");
        }
        if self.entails(!to) {
            let prop_result = self.set_impl(!from, DirectOrigin::ImplicationPropagation(!to));
            assert!(prop_result.is_ok(), "{}", "Inconsistency on the addition of implies({from:?}, {to:?}");
        }
    }

    #[cfg(test)]
    pub fn new_presence_literal(&mut self, scope: Lit) -> Lit {
        let lit = self.new_var(0, 1).geq(1);
        self.add_implication(lit, scope);
        lit
    }

    pub fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> VarRef {
        assert!(
            !self.presence.contains(presence.variable()),
            "The presence literal of an optional variable should not be based on an optional variable"
        );
        let var = self.new_var(lb, ub);
        self.presence.insert(var, presence);
        var
    }

    pub fn presence(&self, term: impl Term) -> Lit {
        self.presence.get(term.variable()).copied().unwrap_or(Lit::TRUE)
    }

    /// Returns `true` if `presence(a) => presence(b)`
    pub fn only_present_with(&self, a: VarRef, b: VarRef) -> bool {
        let prez_a = self.presence(a);
        let prez_b = self.presence(b);
        self.implies(prez_a, prez_b)
    }

    /// Returns true if `a` is known to imply `b`
    pub fn implies(&self, a: Lit, b: Lit) -> bool {
        if self.entails(b) || self.entails(!a) {
            return true;
        }
        self.implications.implies(a, b)
    }

    /// Returns true if `a` and `b` are known to be exclusive
    pub fn exclusive(&self, a: Lit, b: Lit) -> bool {
        // exclusive: !a || !b
        // equivalent to: a => !b
        self.implies(a, !b)
    }

    /// Returns true if we know that two variable are always present jointly.
    pub fn always_present_together(&self, a: VarRef, b: VarRef) -> bool {
        self.presence(a) == self.presence(b)
    }

    /// Returns `true` if the variable is necessarily present and `false` if it is necessarily absent.
    /// Otherwise, the presence status of the variable is unknown and `None` is returned.
    pub fn present(&self, term: impl Term) -> Option<bool> {
        let presence = self.presence(term.variable());
        if self.entails(presence) {
            Some(true)
        } else if self.entails(!presence) {
            Some(false)
        } else {
            None
        }
    }

    /// Returns the domain of an optional variable
    pub fn domain(&self, var: impl Into<VarRef>) -> OptDomain {
        let var = var.into();
        let (lb, ub) = self.bounds(var);
        let prez = self.presence(var);
        match self.value(prez) {
            Some(true) => OptDomain::Present(lb, ub),
            Some(false) => OptDomain::Absent,
            None => OptDomain::Unknown(lb, ub),
        }
    }

    // ============== Integer domain accessors =====================

    pub fn bounds(&self, v: VarRef) -> (IntCst, IntCst) {
        (self.lb(v), self.ub(v))
    }

    pub fn ub(&self, var: impl Into<SignedVar>) -> IntCst {
        self.doms.ub(var)
    }

    pub fn lb(&self, var: impl Into<SignedVar>) -> IntCst {
        self.doms.lb(var)
    }

    /// Returns true if the integer domain of the variable is a singleton or an empty set.
    ///
    /// Note that an empty set is valid for optional variables and implies that
    /// the variable is absent.
    pub fn is_bound(&self, var: VarRef) -> bool {
        self.lb(var) >= self.ub(var)
    }

    pub fn entails(&self, lit: Lit) -> bool {
        debug_assert!(!self.doms.entails(lit) || !self.doms.entails(!lit));
        self.doms.entails(lit)
    }

    pub fn value(&self, lit: Lit) -> Option<bool> {
        if self.entails(lit) {
            Some(true)
        } else if self.entails(!lit) {
            Some(false)
        } else {
            None
        }
    }

    // ============== Updates ==============

    #[inline]
    pub fn decide(&mut self, lit: Lit) -> Result<bool, InvalidUpdate> {
        self.set(lit, Cause::Decision)
    }

    pub fn assume(&mut self, lit: Lit) -> Result<bool, InvalidUpdate> {
        self.set(lit, Cause::Assumption)
    }

    /// Modifies the lower bound of a variable.
    /// The module that made this modification should be identified in the `cause` parameter, which can
    /// be used to query it for an explanation of the change.
    ///
    /// The function returns:
    ///  - `Ok(true)` if the bound was changed and it results in a valid (non-empty) domain.
    ///  - `Ok(false)` if no modification of the domain was carried out. This might occur if the
    ///     provided bound is less constraining than the existing one.
    ///  - `Err(EmptyDomain(v))` if the change resulted in the variable `v` having an empty domain.
    ///     In general, it cannot be assumed that `v` is the same as the variable passed as parameter.
    #[inline]
    pub fn set_lb(&mut self, var: impl Into<SignedVar>, new_lb: IntCst, cause: Cause) -> Result<bool, InvalidUpdate> {
        // var >= lb   <=>    -var <= -lb
        self.set_ub(-var.into(), -new_lb, cause)
    }

    /// Modifies the upper bound of a variable.
    /// The module that made this modification should be identified in the `cause` parameter, which can
    /// be used to query it for an explanation of the change.
    ///
    /// The function returns:
    ///  - `Ok(true)` if the bound was changed and it results in a valid (non-empty) domain
    ///  - `Ok(false)` if no modification of the domain was carried out. This might occur if the
    ///     provided bound is less constraining than the existing one.
    ///  - `Err(EmptyDomain(v))` if the change resulted in the variable `v` having an empty domain.
    ///     In general, it cannot be assumed that `v` is the same as the variable passed as parameter.
    #[inline]
    pub fn set_ub(&mut self, var: impl Into<SignedVar>, new_ub: IntCst, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.set_upper_bound(var.into(), new_ub, cause)
    }

    #[inline]
    pub fn set(&mut self, literal: Lit, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.set_upper_bound(literal.svar(), literal.ub_value(), cause)
    }

    #[inline]
    fn set_impl(&mut self, literal: Lit, cause: DirectOrigin) -> Result<bool, InvalidUpdate> {
        self.set_upper_bound_impl(literal.svar(), literal.ub_value(), Origin::Direct(cause))
    }

    pub fn set_upper_bound(&mut self, affected: SignedVar, ub: IntCst, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.set_upper_bound_impl(affected, ub, cause.into())
    }

    fn set_upper_bound_impl(&mut self, affected: SignedVar, ub: IntCst, cause: Origin) -> Result<bool, InvalidUpdate> {
        match self.presence(affected.variable()) {
            Lit::TRUE => self.set_upper_bound_non_optional(affected, ub, cause),
            _ => self.set_bound_optional(affected, ub, cause),
        }
    }

    fn set_bound_optional(
        &mut self,
        affected: SignedVar,
        new_ub: IntCst,
        cause: Origin,
    ) -> Result<bool, InvalidUpdate> {
        let prez = self.presence(affected.variable());
        // variable must be optional
        debug_assert_ne!(prez, Lit::TRUE);
        // invariant: optional variable cannot be involved in implications
        debug_assert!(self
            .implications
            .direct_implications_of(affected.leq(new_ub))
            .next()
            .is_none());

        let new_bound = affected.leq(new_ub);

        if self.entails(!prez) {
            // variable is absent, we do nothing
            Ok(false)
        } else if !self.doms.entails(!new_bound) {
            // variable is not proven absent and this is a valid update
            let res = self.doms.set_upper_bound(affected, new_ub, cause);
            debug_assert!(res.is_ok());
            // either valid update or noop
            res
        } else {
            // invalid update, set the variable to absent
            let origin = match cause {
                Origin::Direct(direct) => direct,
                Origin::PresenceOfEmptyDomain(_, _) => unreachable!(),
            };
            let not_prez = !prez;
            self.set_upper_bound_non_optional(
                not_prez.svar(),
                not_prez.ub_value(),
                Origin::PresenceOfEmptyDomain(new_bound, origin),
            )
        }
    }

    fn set_upper_bound_non_optional(
        &mut self,
        affected: SignedVar,
        new_ub: IntCst,
        cause: Origin,
    ) -> Result<bool, InvalidUpdate> {
        // remember the top of the event stack
        let mut cursor = self.trail().reader();
        cursor.move_to_end(self.trail());

        debug_assert_eq!(self.presence(affected.variable()), Lit::TRUE);

        // variable is necessarily present, perform update
        let res = self.doms.set_upper_bound(affected, new_ub, cause);
        match res {
            Ok(true) => {
                // exactly one domain change must have occurred
                debug_assert_eq!(cursor.num_pending(self.trail()), 1);
                // we need to propagate the implications, go through all event that have occurred since we entered
                // this method

                while let Some(ev) = cursor.pop(self.trail()) {
                    let lit = ev.new_literal();
                    // invariant: variables in implications are not optional
                    debug_assert_eq!(self.presence(lit.variable()), Lit::TRUE);
                    for implied in self.implications.direct_implications_of(lit) {
                        self.doms.set_upper_bound(
                            implied.svar(),
                            implied.ub_value(),
                            Origin::implication_propagation(lit),
                        )?;
                    }
                }
                // we propagated everything without any error, we are good to go
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(InvalidUpdate(lit, fail_cause)) => {
                debug_assert_eq!(lit, affected.leq(new_ub));
                debug_assert_eq!(fail_cause, cause);
                Err(InvalidUpdate(lit, fail_cause))
            }
        }
    }

    #[inline]
    pub fn set_unchecked(&mut self, literal: Lit, cause: Cause) {
        // todo: to have optimal performance, we should implement the unchecked version in IntDomains
        let res = self.set(literal, cause);
        debug_assert!(res.is_ok());
    }

    pub fn set_bound_unchecked(&mut self, affected: SignedVar, new_ub: IntCst, cause: Cause) {
        // todo: to have optimal performance, we should implement the unchecked version in IntDomains
        let res = self.set_upper_bound(affected, new_ub, cause);
        debug_assert!(res.is_ok());
    }

    // ============= Variables =================

    pub fn variables(&self) -> impl Iterator<Item = VarRef> {
        self.doms.variables()
    }

    pub fn bound_variables(&self) -> impl Iterator<Item = (VarRef, IntCst)> + '_ {
        self.doms.bound_variables()
    }

    // history

    /// Returns the index of the first event that makes `lit` true.
    /// If the function returns None, it means that `lit` was true at the root level.
    pub fn implying_event(&self, lit: Lit) -> Option<EventIndex> {
        self.doms.implying_event(lit)
    }

    pub fn num_events(&self) -> u32 {
        self.doms.num_events()
    }

    pub fn last_event(&self) -> Option<&Event> {
        self.doms.last_event()
    }

    pub fn trail(&self) -> &ObsTrail<Event> {
        self.doms.trail()
    }

    pub fn entailing_level(&self, lit: Lit) -> DecLvl {
        debug_assert!(self.entails(lit));
        match self.implying_event(lit) {
            Some(loc) => self.trail().decision_level(loc),
            None => DecLvl::ROOT,
        }
    }

    pub fn get_event(&self, loc: EventIndex) -> &Event {
        self.trail().get_event(loc)
    }

    // State management

    pub fn undo_last_event(&mut self) -> Origin {
        self.doms.undo_last_event()
    }

    // ================== Explanation ==============

    /// Given an invalid update of the literal `l`, derives a clause `(l_1 & l_2 & ... & l_n) => !l_dec`
    /// where:
    ///
    ///  - the literals `l_i` are entailed at the previous decision level of the current state,
    ///  - the literal `l_dec` is the decision that was taken at the current decision level.
    ///
    /// The update of `l` must not directly originate from a decision as it is necessarily the case that
    /// `!l` holds in the current state. It is thus considered a logic error to impose an obviously wrong decision.
    ///
    /// It can, however, directly originate from an assumption (in which case we are necessarily UNSAT, by the way).
    pub fn clause_for_invalid_update(&mut self, failed: InvalidUpdate, explainer: &mut impl Explainer) -> Conflict {
        let InvalidUpdate(literal, cause) = failed;
        debug_assert!(!self.entails(literal));

        // an update is invalid iff its negation holds AND the affected variable is present
        debug_assert!(self.entails(!literal));
        debug_assert!(self.entails(self.presence(literal.variable())));

        // the base of the explanation is  `!literal & literal & prez(literal) -> false`

        let mut explanation = Explanation::with_capacity(2);
        explanation.push(!literal);
        explanation.push(self.presence(literal));

        if cause != Origin::ASSUMPTION {
            // However, `literal` does not hold in the current state and we need to replace it.
            // Thus we replace it with a set of literals `x_1 & ... & x_m` such that
            // `x_1 & ... & x_m -> literal`
            self.add_implying_literals_to_explanation(literal, cause, &mut explanation, explainer);
        }
        debug_assert!(explanation.lits.iter().copied().all(|l| self.entails(l)));

        // now all disjuncts hold in the current state
        // we then transform this clause to be in the first unique implication point (1UIP) form.
        self.refine_explanation(explanation, explainer)
    }

    /// Refines an explanation into an asserting clause.
    ///
    /// Note that a partial backtrack (within the current decision level) will occur in the process.
    /// This is necessary to provide explainers with the exact state in which their decisions were made.
    pub fn refine_explanation(&mut self, explanation: Explanation, explainer: &mut impl Explainer) -> Conflict {
        debug_assert!(explanation.literals().iter().all(|&l| self.entails(l)));
        let mut explanation = explanation;

        // literals falsified at the current decision level, we need to proceed until there is a single one left (1UIP)
        self.queue.clear();
        // literals that are beyond the current decision level and will be part of the final clause
        let mut result: DisjunctionBuilder = DisjunctionBuilder::with_capacity(32);

        let decision_level = self.current_decision_level();
        let mut resolved = LitSet::new();
        let clause: Disjunction = loop {
            for l in explanation.lits.drain(..) {
                debug_assert!(self.entails(l));
                // find the location of the event that made it true
                // if there is no such event, it means that the literal is implied in the initial state and we can ignore it
                if let Some(loc) = self.implying_event(l) {
                    match self.trail().decision_level_class(loc) {
                        DecisionLevelClass::Root => {
                            // implied at decision level 0, and thus always true, discard it
                        }
                        DecisionLevelClass::Current => {
                            // at the current decision level, add to the queue
                            self.queue.push(loc, l)
                        }
                        DecisionLevelClass::Intermediate => {
                            // implied before the current decision level, the negation of the literal will appear in the final clause (1UIP)
                            result.push(!l)
                        }
                    }
                }
            }
            debug_assert!(explanation.lits.is_empty());
            if self.queue.is_empty() {
                // queue is empty, which means that all literal in the clause will be below the current decision level
                // this can happen if
                // - we had a lazy propagator that did not immediately detect the inconsistency
                // - we are at decision level 0

                // if we were at the root decision level, we should have derived the empty clause
                debug_assert!(decision_level != DecLvl::ROOT || result.is_empty());
                break result.into();
            }
            debug_assert!(!self.queue.is_empty());

            // not reached the first UIP yet,
            // select latest falsified literal from queue
            let (l, l_cause) = self.queue.pop().unwrap();

            if self.queue.is_empty() {
                // We have reached the first Unique Implication Point (UIP)
                // the content of result is a conjunction of literal that imply `!l`
                // build the conflict clause and exit
                debug_assert!(self.queue.is_empty());
                result.push(!l);
                break result.into();
            }

            debug_assert!(l_cause < self.trail().next_slot());
            debug_assert!(self.entails(l));
            let mut cause = None;
            // backtrack until the latest falsifying event
            // this will undo some of the changes but will keep us in the same decision level
            while l_cause < self.trail().next_slot() {
                // the event cannot be a decision, because it would have been detected as a UIP earlier
                debug_assert_ne!(self.last_event().unwrap().cause, Origin::DECISION);
                let x = self.undo_last_event();
                cause = Some(x);
            }
            let cause = cause.unwrap();

            resolved.insert(l);
            // in the explanation, add a set of literal whose conjunction implies `l.lit`
            self.add_implying_literals_to_explanation(l, cause, &mut explanation, explainer);
        };

        // minimize the learned clause (removal of redundant literals)
        let clause = minimize::minimize_clause(clause, self, explainer);

        // when debugging check that the learnt clause would not prune any witness solution
        #[cfg(debug_assertions)]
        debug_assert!(!witness::pruned_by_clause(&clause), "Post minimization: {clause:?}");

        Conflict { clause, resolved }
    }

    fn extract_assumptions_implying(
        &mut self,
        explanation: &mut Explanation,
        explainer: &mut impl Explainer,
    ) -> Explanation {
        debug_assert!(explanation.lits.iter().all(|&l| self.entails(l)));
        let mut result = Explanation::new();

        self.queue.clear();

        loop {
            for l in explanation.lits.drain(..) {
                if let Some(loc) = self.implying_event(l) {
                    let ev = self.trail().get_event(loc);
                    if ev.cause == Origin::ASSUMPTION {
                        result.lits.push(ev.new_literal());
                    } else {
                        debug_assert!(self.entails(l));
                        self.queue.push(loc, l);
                    }
                }
            }
            debug_assert!(explanation.lits.is_empty());

            if self.queue.is_empty() {
                break;
            }
            let (lit, _) = self.queue.pop().unwrap();

            if let Some(implying_lits) = self.implying_literals(lit, explainer) {
                explanation.lits.extend(implying_lits);
            }
        }
        result
    }

    pub fn extract_unsat_core_after_invalid_update(
        &mut self,
        failed: InvalidUpdate,
        explainer: &mut impl Explainer,
    ) -> Explanation {
        let InvalidUpdate(literal, cause) = failed;
        debug_assert!(!self.entails(literal));
        let mut explanation = Explanation::new();
        explanation.lits = self
            .clause_for_invalid_update(failed, explainer)
            .clause
            .literals()
            .iter()
            .map(|&l| !l)
            .collect();

        let mut unsat_core = self.extract_assumptions_implying(&mut explanation, explainer);
        if cause == Origin::ASSUMPTION {
            unsat_core.lits.push(literal);
        }
        unsat_core
    }

    pub fn extract_unsat_core_after_conflict(
        &mut self,
        conflict: Conflict,
        explainer: &mut impl Explainer,
    ) -> Explanation {
        let mut explanation = Explanation::new();
        explanation.lits = conflict.clause.literals().iter().map(|&l| !l).collect();
        self.extract_assumptions_implying(&mut explanation, explainer)
    }

    /// Returns all decisions that were taken since the root decision level.
    pub fn decisions(&self) -> Vec<(DecLvl, Lit)> {
        let mut decs = Vec::new();
        let mut lvl = DecLvl::ROOT + 1;
        for e in self.trail().events() {
            if e.cause == Origin::DECISION {
                decs.push((lvl, e.new_literal()));
                lvl += 1;
            }
        }

        decs
    }

    /// Computes literals `l_1 ... l_n` such that:
    ///  - `l_1 & ... & l_n => literal`
    ///  - each `l_i` is entailed at the current level.
    ///
    /// Assumptions:
    ///  - `literal` is not entailed in the current state
    ///  - `cause` provides the explanation for asserting `literal` (and is not a decision).
    pub(crate) fn add_implying_literals_to_explanation(
        &self,
        literal: Lit,
        cause: Origin,
        explanation: &mut Explanation,
        explainer: &mut impl Explainer,
    ) {
        let state = DomainsSnapshot::current(self);
        Domains::add_implying_literals_to_explanation_impl(&state, literal, cause, explanation, explainer)
    }

    fn add_implying_literals_to_explanation_impl(
        state: &DomainsSnapshot,
        literal: Lit,
        cause: Origin,
        explanation: &mut Explanation,
        explainer: &mut dyn Explainer,
    ) {
        // we should be in a state where the literal is not true yet, but immediately implied
        debug_assert!(!state.entails(literal));
        match cause {
            Origin::Direct(DirectOrigin::Decision | DirectOrigin::Assumption | DirectOrigin::Encoding) => panic!(),
            Origin::Direct(DirectOrigin::ExternalInference(cause)) => {
                // ask for a clause (l1 & l2 & ... & ln) => lit
                explainer.explain(cause, literal, state, explanation);
            }
            Origin::Direct(DirectOrigin::ImplicationPropagation(causing_literal)) => explanation.push(causing_literal),
            Origin::PresenceOfEmptyDomain(invalid_lit, cause) => {
                // invalid_lit & !invalid_lit => absent(variable(invalid_lit))
                debug_assert!(state.entails(!invalid_lit));
                explanation.push(!invalid_lit);
                match cause {
                    DirectOrigin::Decision | DirectOrigin::Assumption | DirectOrigin::Encoding => {
                        explanation.push(invalid_lit);
                    }
                    DirectOrigin::ExternalInference(cause) => {
                        // ask for a clause (l1 & l2 & ... & ln) => lit
                        explainer.explain(cause, invalid_lit, state, explanation);
                    }
                    DirectOrigin::ImplicationPropagation(causing_literal) => {
                        explanation.push(causing_literal);
                    }
                }
            }
        }
    }

    /// For a literal `l` that is true in the current state, returns a list of entailing literals `l_1 ... l_n`
    /// that forms an explanation `(l_1 & ... l_n) => l`.
    /// Returns None if the literal is a decision.
    ///
    /// Limitation: differently from the explanations provided in the main clause construction loop,
    /// the explanation will not be built in the exact state where the inference was made (which might be problematic
    /// for some reasoners).
    pub fn implying_literals(&self, literal: Lit, explainer: &mut dyn Explainer) -> Option<Vec<Lit>> {
        // we should be in a state where the literal is true
        debug_assert!(self.entails(literal));
        let event = if let Some(event) = self.implying_event(literal) {
            event
        } else {
            // event is always true (entailed at root), and does have any implying literals
            return Some(Vec::new());
        };
        let event = self.get_event(event);
        let mut explanation = Explanation::new();

        if matches!(
            event.cause,
            Origin::Direct(DirectOrigin::Decision | DirectOrigin::Assumption | DirectOrigin::Encoding)
        ) {
            None
        } else {
            let state = &DomainsSnapshot::preceding(self, literal);
            Domains::add_implying_literals_to_explanation_impl(
                state,
                literal,
                event.cause,
                &mut explanation,
                explainer,
            );

            Some(explanation.lits)
        }
    }

    /// A literal `l1` normally represent the  fact   `l1=T v l1=ø`
    /// If we have a literal  `l2  <-> l1=ø`    (negation of its presence literal)
    /// Then  `l1 v l2`  is logically equivalent to `l1`
    /// This function returns true if the two literals are in this relationship, i.e., one represents the absence of the other
    pub fn fusable(&self, l1: Lit, l2: Lit) -> bool {
        l1 == !self.presence(l2) || l2 == !self.presence(l1)
    }
}

impl Default for Domains {
    fn default() -> Self {
        Self::new()
    }
}

impl Backtrack for Domains {
    fn save_state(&mut self) -> DecLvl {
        self.doms.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.doms.num_saved()
    }

    fn restore_last(&mut self) {
        self.doms.restore_last()
    }
}

/// Data resulting from a conflict, of which the most important is the learnt clause.
pub struct Conflict {
    /// Clause associated to the conflict.
    /// A set of literals of which at least one must be true to avoid the conflict.
    pub clause: Disjunction,
    /// Resolved literals that participate in the conflict.
    /// Those literals appeared in an explanation when producing the associated clause, but
    /// where replaced by their own explanation (and thus do not appear in the clause).
    /// Those are typically exploited by some branching heuristics (e.g. LRB) to identify
    /// literals participating in conflicts.
    pub resolved: LitSet,
}

impl Conflict {
    /// NUmber of literals in the associated clause
    pub fn len(&self) -> usize {
        self.clause.len()
    }

    /// True if the associated clause is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the literals in the clause
    pub fn literals(&self) -> &[Lit] {
        self.clause.literals()
    }

    /// Returns a new conflict that is a contraction (i.e. can never be avoided).
    /// Here, a conflict with an empty clause.
    pub fn contradiction() -> Self {
        Conflict {
            clause: Disjunction::new(Vec::new()),
            resolved: Default::default(),
        }
    }
}

/// Custom debug: the immense majority of the time we are only interested in seeing the clause.
impl Debug for Conflict {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.clause)
    }
}

/// A term that can be converted into a variable.
/// Notably implemented for `VarRef`, `Lit`, `IVar`, `SVar`, `BVar`
pub trait Term {
    fn variable(self) -> VarRef;
}
impl Term for Lit {
    fn variable(self) -> VarRef {
        self.variable()
    }
}
impl Term for SignedVar {
    fn variable(self) -> VarRef {
        self.variable()
    }
}
impl<T: Into<VarRef>> Term for T {
    fn variable(self) -> VarRef {
        self.into()
    }
}

#[cfg(test)]
mod tests {
    use crate::backtrack::Backtrack;
    use crate::core::state::domains::Domains;
    use crate::core::state::*;
    use crate::core::*;
    use crate::reasoners::ReasonerId;
    use std::collections::HashSet;

    #[test]
    fn test_optional() {
        let mut domains = Domains::default();
        let p1 = domains.new_presence_literal(Lit::TRUE);
        // p2 is present if p1 is true
        let p2 = domains.new_presence_literal(p1);
        // i is present if p2 is true
        let i = domains.new_optional_var(0, 10, p2);

        let check_doms = |domains: &Domains, lp1, up1, lp2, up2, li, ui| {
            assert_eq!(domains.bounds(p1.variable()), (lp1, up1));
            assert_eq!(domains.bounds(p2.variable()), (lp2, up2));
            assert_eq!(domains.bounds(i), (li, ui));
        };
        check_doms(&domains, 0, 1, 0, 1, 0, 10);

        // reduce domain of i to [5,5]
        assert_eq!(domains.set_lb(i, 5, Cause::Decision), Ok(true));
        check_doms(&domains, 0, 1, 0, 1, 5, 10);
        assert_eq!(domains.set_ub(i, 5, Cause::Decision), Ok(true));
        check_doms(&domains, 0, 1, 0, 1, 5, 5);

        // make the domain of i empty, this should imply that p2 = false
        assert_eq!(domains.set_lb(i, 6, Cause::Decision), Ok(true));
        check_doms(&domains, 0, 1, 0, 0, 5, 5);

        // make p1 = true, this should have no impact on the rest
        assert_eq!(domains.set(p1, Cause::Decision), Ok(true));
        check_doms(&domains, 1, 1, 0, 0, 5, 5);

        // make p2 have an empty domain, this should imply that p1 = false which is a contradiction with our previous decision
        assert!(matches!(domains.set(p2, Cause::Decision), Err(InvalidUpdate(_, _))));
    }

    #[test]
    fn test_presence_relations() {
        let mut domains = Domains::new();
        let p = domains.new_var(0, 1);
        let p1 = domains.new_optional_var(0, 1, p.geq(1));
        let p2 = domains.new_optional_var(0, 1, p.geq(1));

        assert!(domains.always_present_together(p1, p2));
        assert!(!domains.always_present_together(p, p1));
        assert!(!domains.always_present_together(p, p2));

        assert!(domains.always_present_together(p, p));
        assert!(domains.only_present_with(p, p));
        assert!(domains.always_present_together(p1, p1));
        assert!(domains.only_present_with(p1, p1));

        assert!(domains.only_present_with(p1, p));
        assert!(domains.only_present_with(p2, p));
        assert!(domains.only_present_with(p1, p2));
        assert!(domains.only_present_with(p2, p1));
        assert!(!domains.only_present_with(p, p1));
        assert!(!domains.only_present_with(p, p2));

        let x = domains.new_var(0, 1);
        let x1 = domains.new_optional_var(0, 1, x.geq(1));

        assert!(domains.only_present_with(x1, x));
        assert!(!domains.only_present_with(x, x1));

        // two top level vars
        assert!(domains.always_present_together(p, x));
        assert!(domains.only_present_with(p1, x));
        assert!(domains.only_present_with(x1, p));

        assert!(!domains.only_present_with(p1, x1));
        assert!(!domains.only_present_with(x1, p1));
    }

    #[test]
    fn domain_updates() {
        let mut model = Domains::new();
        let a = model.new_var(0, 10);

        assert_eq!(model.set_lb(a, -1, Cause::Decision), Ok(false));
        assert_eq!(model.set_lb(a, 0, Cause::Decision), Ok(false));
        assert_eq!(model.set_lb(a, 1, Cause::Decision), Ok(true));
        assert_eq!(model.set_ub(a, 11, Cause::Decision), Ok(false));
        assert_eq!(model.set_ub(a, 10, Cause::Decision), Ok(false));
        assert_eq!(model.set_ub(a, 9, Cause::Decision), Ok(true));
        // domain is [1, 9]
        assert_eq!(model.bounds(a), (1, 9));

        model.save_state();
        assert_eq!(model.set_lb(a, 9, Cause::Decision), Ok(true));
        assert_eq!(
            model.set_lb(a, 10, Cause::Decision),
            Err(InvalidUpdate(Lit::geq(a, 10), Origin::DECISION))
        );

        model.restore_last();
        assert_eq!(model.bounds(a), (1, 9));
        assert_eq!(model.set_ub(a, 1, Cause::Decision), Ok(true));
        assert_eq!(
            model.set_ub(a, 0, Cause::Decision),
            Err(InvalidUpdate(Lit::leq(a, 0), Origin::DECISION))
        );
    }

    #[test]
    fn test_explanation() {
        let mut model = Domains::new();
        let a = Lit::geq(model.new_var(0, 1), 1);
        let b = Lit::geq(model.new_var(0, 1), 1);
        let n = model.new_var(0, 10);

        // constraint 0: "a => (n <= 4)"
        // constraint 1: "b => (n >= 5)"

        let writer = ReasonerId::Sat;

        let cause_a = Cause::inference(writer, 0u32);
        let cause_b = Cause::inference(writer, 1u32);

        #[allow(unused_must_use)]
        let propagate = |model: &mut Domains| -> Result<bool, InvalidUpdate> {
            if model.entails(a) {
                model.set_ub(n, 4, cause_a)?;
            }
            if model.entails(b) {
                model.set_lb(n, 5, cause_b)?;
            }
            Ok(true)
        };

        struct Expl {
            a: Lit,
            b: Lit,
            n: VarRef,
        }
        impl Explainer for Expl {
            fn explain(
                &mut self,
                cause: InferenceCause,
                literal: Lit,
                _model: &DomainsSnapshot,
                explanation: &mut Explanation,
            ) {
                assert_eq!(cause.writer, ReasonerId::Sat);
                match cause.payload {
                    0 => {
                        assert_eq!(literal, Lit::leq(self.n, 4));
                        explanation.push(self.a);
                    }
                    1 => {
                        assert_eq!(literal, Lit::geq(self.n, 5));
                        explanation.push(self.b);
                    }
                    _ => panic!("unexpected payload"),
                }
            }
        }

        let mut network = Expl { a, b, n };

        propagate(&mut model).unwrap();
        model.save_state();
        model.decide(a).unwrap();
        assert_eq!(model.bounds(a.variable()), (1, 1));
        propagate(&mut model).unwrap();
        assert_eq!(model.domain(n), OptDomain::Present(0, 4));
        model.save_state();
        model.set_lb(n, 1, Cause::Decision).unwrap();
        model.save_state();
        model.decide(b).unwrap();
        let err = match propagate(&mut model) {
            Err(err) => err,
            _ => panic!(),
        };

        let clause = model.clause_for_invalid_update(err, &mut network);
        let clause: HashSet<_> = clause.literals().iter().copied().collect();

        // we have three rules
        //  -  !(n <= 4) || !(n >= 5)   (conflict)
        //  -  !a || (n <= 4)           (clause a)
        //  -  !b || (n >= 5)           (clause b)
        // Explanation should perform resolution of the first and last rules for the literal (n >= 5):
        //   !(n <= 4) || !b
        //   !b || (n > 4)      (equivalent to previous)
        let mut expected = HashSet::new();
        expected.insert(!b);
        expected.insert(Lit::gt(n, 4));
        assert_eq!(clause, expected);
    }

    #[test]
    fn test_optional_propagation_error() {
        let mut model = Domains::new();
        let p = model.new_var(0, 1);
        let i = model.new_optional_var(0, 10, p.geq(1));
        let x = model.new_var(0, 10);

        model.save_state();
        assert_eq!(model.set_lb(p, 1, Cause::Decision), Ok(true));
        model.save_state();
        assert_eq!(model.set_ub(i, 5, Cause::Decision), Ok(true));

        // irrelevant event
        model.save_state();
        assert_eq!(model.set_ub(x, 5, Cause::Decision), Ok(true));

        model.save_state();
        assert!(model.set_lb(i, 6, Cause::Decision).is_err());
    }

    #[test]
    fn test_unsat_core_extraction_bool() {
        let mut model = Domains::new();
        let a = Lit::geq(model.new_var(0, 1), 1);
        let b = Lit::geq(model.new_var(0, 1), 1);
        let c = Lit::geq(model.new_var(0, 1), 1);
        let d = Lit::geq(model.new_var(0, 1), 1);
        let e = Lit::geq(model.new_var(0, 1), 1);
        let f = Lit::geq(model.new_var(0, 1), 1);
        let g = Lit::geq(model.new_var(0, 1), 1);
        let h = Lit::geq(model.new_var(0, 1), 1);

        // assumptions: a, b, d
        // expected core: a, b

        // constraint 0: "a => c"
        // constraint 1: "b & c => f"
        // constraint 2: "f => !h"
        // constraint 3: "d => e"
        // constraint 4: "g => h"

        // a => c | b => f => !h | d => e | g => h |
        //      '========^

        let writer = ReasonerId::Sat;

        let cause_a = Cause::inference(writer, 0u32);
        let cause_bc = Cause::inference(writer, 1u32);
        let cause_f = Cause::inference(writer, 2u32);
        let cause_e = Cause::inference(writer, 3u32);
        let cause_g = Cause::inference(writer, 4u32);

        #[allow(unused_must_use)]
        let propagate = |model: &mut Domains| -> Result<bool, InvalidUpdate> {
            if model.entails(a) {
                model.set(c, cause_a)?;
            }
            if model.entails(b) & model.entails(c) {
                model.set(f, cause_bc)?;
            }
            if model.entails(f) {
                model.set(h.not(), cause_f)?;
            }
            if model.entails(d) {
                model.set(e, cause_e)?;
            }
            if model.entails(g) {
                model.set(h, cause_g)?;
            }
            Ok(true)
        };

        struct Expl {
            a: Lit,
            b: Lit,
            c: Lit,
            d: Lit,
            e: Lit,
            f: Lit,
            g: Lit,
            h: Lit,
        }
        impl Explainer for Expl {
            fn explain(
                &mut self,
                cause: InferenceCause,
                literal: Lit,
                _model: &DomainsSnapshot,
                explanation: &mut Explanation,
            ) {
                assert_eq!(cause.writer, ReasonerId::Sat);
                match cause.payload {
                    0 => {
                        assert_eq!(literal, self.c);
                        explanation.push(self.a);
                    }
                    1 => {
                        assert_eq!(literal, self.f);
                        explanation.push(self.b);
                        explanation.push(self.c);
                    }
                    2 => {
                        assert_eq!(literal, self.h.not());
                        explanation.push(self.f);
                    }
                    3 => {
                        assert_eq!(literal, self.e);
                        explanation.push(self.d);
                    }
                    4 => {
                        assert_eq!(literal, self.h);
                        explanation.push(self.g);
                    }
                    _ => panic!("unexpected payload"),
                }
            }
        }

        let mut network = Expl { a, b, c, d, e, f, g, h };

        propagate(&mut model).unwrap();

        model.save_state();
        assert!(model.assume(a).unwrap());
        assert_eq!(model.bounds(a.variable()), (1, 1));
        propagate(&mut model).unwrap();
        assert_eq!(model.bounds(c.variable()), (1, 1));

        model.save_state();
        assert!(model.assume(b).unwrap());
        assert_eq!(model.bounds(b.variable()), (1, 1));
        propagate(&mut model).unwrap();
        assert_eq!(model.bounds(f.variable()), (1, 1));
        assert_eq!(model.bounds(h.variable()), (0, 0));

        model.save_state();
        assert!(model.assume(d).unwrap());
        assert_eq!(model.bounds(d.variable()), (1, 1));
        propagate(&mut model).unwrap();
        assert_eq!(model.bounds(e.variable()), (1, 1));

        model.save_state();
        model.decide(g).unwrap();
        assert_eq!(model.bounds(g.variable()), (1, 1));
        let err = match propagate(&mut model) {
            Err(err) => err,
            _ => panic!(),
        };

        let conflict = model.clause_for_invalid_update(err, &mut network);
        let unsat_core = model.extract_unsat_core_after_conflict(conflict, &mut network).lits;
        let unsat_core_set: HashSet<Lit> = unsat_core.iter().copied().collect();

        let mut expected = HashSet::new();
        expected.insert(a);
        expected.insert(b);
        assert_eq!(unsat_core_set, expected);

        model.restore_last();

        model.save_state();
        model.assume(g).unwrap();
        assert_eq!(model.bounds(g.variable()), (1, 1));
        let err = match propagate(&mut model) {
            Err(err) => err,
            _ => panic!(),
        };

        let unsat_core = model.extract_unsat_core_after_invalid_update(err, &mut network).lits;
        let unsat_core_set: HashSet<Lit> = unsat_core.iter().copied().collect();

        let mut expected = HashSet::new();
        expected.insert(a);
        expected.insert(b);
        expected.insert(g);
        assert_eq!(unsat_core_set, expected);

        model.restore_last();

        model.save_state();
        let err = model.assume(h).unwrap_err();

        let unsat_core = model.extract_unsat_core_after_invalid_update(err, &mut network).lits;
        let unsat_core_set: HashSet<Lit> = unsat_core.iter().copied().collect();

        let mut expected = HashSet::new();
        expected.insert(a);
        expected.insert(b);
        expected.insert(h);
        assert_eq!(unsat_core_set, expected);
    }

    #[test]
    fn test_unsat_core_extraction_int() {
        let mut model = Domains::new();
        let x = model.new_var(0, 10);
        let y = model.new_var(0, 10);

        // assumptions: [x <= 3], [y <= 4]
        // constraint: [x <= 5] => [y >= 6]

        let writer = ReasonerId::Sat;

        let cause_xleq5 = Cause::inference(writer, 0u32);

        #[allow(unused_must_use)]
        let propagate = |model: &mut Domains| -> Result<bool, InvalidUpdate> {
            if model.entails(x.leq(5)) {
                model.set(y.geq(6), cause_xleq5)?;
            }
            Ok(true)
        };

        struct Expl {
            x: VarRef,
            y: VarRef,
        }
        impl Explainer for Expl {
            fn explain(
                &mut self,
                cause: InferenceCause,
                literal: Lit,
                _model: &DomainsSnapshot,
                explanation: &mut Explanation,
            ) {
                assert_eq!(cause.writer, ReasonerId::Sat);
                match cause.payload {
                    0 => {
                        assert_eq!(literal, !(self.y.leq(4))); // i.e. y.geq(5)
                        explanation.push(self.x.leq(5));
                    }
                    _ => panic!("unexpected payload"),
                }
            }
        }

        let mut network = Expl { x, y };

        propagate(&mut model).unwrap();

        model.save_state();
        assert!(model.assume(x.leq(3)).unwrap());
        assert_eq!(model.bounds(x.variable()), (0, 3));
        propagate(&mut model).unwrap();
        assert_eq!(model.bounds(y.variable()), (6, 10));

        model.save_state();
        let err = model.assume(y.leq(4)).unwrap_err();

        let unsat_core = model.extract_unsat_core_after_invalid_update(err, &mut network).lits;
        let unsat_core_set: HashSet<Lit> = unsat_core.iter().copied().collect();

        let mut expected = HashSet::new();
        expected.insert(x.leq(3)); // Previously, an unfixed bug would result in [x <= 5] instead of the "actual" assumption [x <= 3]
        expected.insert(y.leq(4));
        assert_eq!(unsat_core_set, expected);
    }
}
