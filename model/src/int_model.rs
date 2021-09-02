mod cause;
pub mod domains;
pub mod event;
mod explanation;
mod int_domains;
mod presence_graph;

pub use explanation::*;

pub use cause::{Cause, InferenceCause};

use crate::bounds::{Disjunction, Lit, Relation};
use crate::int_model::cause::{DirectOrigin, Origin};
use crate::int_model::domains::OptDomains;
use crate::int_model::event::Event;
use crate::lang::{IntCst, VarRef};
use crate::Label;
use aries_backtrack::DecLvl;
use aries_backtrack::{Backtrack, DecisionLevelClass, EventIndex, ObsTrail};
use aries_collections::ref_store::RefVec;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct IntDomain {
    pub lb: IntCst,
    pub ub: IntCst,
}
impl IntDomain {
    pub fn new(lb: IntCst, ub: IntCst) -> IntDomain {
        IntDomain { lb, ub }
    }

    pub fn size(&self) -> i64 {
        (self.ub as i64) - (self.lb as i64)
    }

    pub fn is_bound(&self) -> bool {
        self.lb == self.ub
    }

    pub fn is_empty(&self) -> bool {
        self.lb > self.ub
    }
}

impl std::fmt::Display for IntDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_bound() {
            write!(f, "{}", self.lb)
        } else if self.is_empty() {
            write!(f, "none")
        } else {
            write!(f, "[{}, {}]", self.lb, self.ub)
        }
    }
}

/// Represents a triggered event of setting a conflicting literal.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct InvalidUpdate(pub Lit, pub Origin);

#[derive(Clone)]
pub struct DiscreteModel {
    labels: RefVec<VarRef, Label>,
    pub domains: OptDomains,
    /// A working queue used when building explanations
    queue: BinaryHeap<InQueueLit>,
}

impl DiscreteModel {
    pub fn new() -> DiscreteModel {
        let mut model = DiscreteModel {
            labels: Default::default(),
            domains: Default::default(),
            queue: Default::default(),
        };
        let zero = model.labels.push("ZERO".into());
        debug_assert_eq!(VarRef::ZERO, zero);
        model
    }

    pub fn new_var<L: Into<Label>>(&mut self, lb: IntCst, ub: IntCst, label: L) -> VarRef {
        let id1 = self.labels.push(label.into());
        let id2 = self.domains.new_var(lb, ub);
        debug_assert_eq!(id1, id2);
        id1
    }
    pub fn new_optional_var<L: Into<Label>>(&mut self, lb: IntCst, ub: IntCst, presence: Lit, label: L) -> VarRef {
        let id1 = self.labels.push(label.into());
        let id2 = self.domains.new_optional_var(lb, ub, presence);
        debug_assert_eq!(id1, id2);
        id1
    }

    pub fn new_presence_var(&mut self, scope: Lit, label: impl Into<Label>) -> VarRef {
        let id1 = self.labels.push(label.into());
        let id2 = self.domains.new_presence_literal(scope);
        debug_assert_eq!(id1, id2.variable());
        id1
    }

    pub fn variables(&self) -> impl Iterator<Item = VarRef> {
        self.labels.keys()
    }

    pub fn label(&self, var: impl Into<VarRef>) -> Option<&str> {
        self.labels[var.into()].get()
    }

    pub fn lb(&self, var: impl Into<VarRef>) -> IntCst {
        self.domains.lb(var.into())
    }

    pub fn ub(&self, var: impl Into<VarRef>) -> IntCst {
        self.domains.ub(var.into())
    }

    pub fn domain_of(&self, var: impl Into<VarRef>) -> (IntCst, IntCst) {
        self.domains.bounds(var.into())
    }

    pub fn decide(&mut self, literal: Lit) -> Result<bool, InvalidUpdate> {
        match literal.relation() {
            Relation::Leq => self.set_ub(literal.variable(), literal.value(), Cause::Decision),
            Relation::Gt => self.set_lb(literal.variable(), literal.value() + 1, Cause::Decision),
        }
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
    pub fn set_lb(&mut self, var: impl Into<VarRef>, lb: IntCst, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.domains.set_lb(var.into(), lb, cause)
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
    pub fn set_ub(&mut self, var: impl Into<VarRef>, ub: IntCst, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.domains.set_ub(var.into(), ub, cause)
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
    pub fn clause_for_invalid_update(&mut self, failed: InvalidUpdate, explainer: &mut impl Explainer) -> Disjunction {
        let InvalidUpdate(literal, cause) = failed;
        debug_assert!(!self.entails(literal));

        // an update is invalid iff its negation holds AND the affected variable is present
        debug_assert!(self.entails(!literal));
        debug_assert!(self.entails(self.domains.presence(literal.variable())));

        // the base of the explanation is `(!literal v literal)`.

        let mut explanation = Explanation::with_capacity(2);
        explanation.push(!literal);

        // However, `literal` does not hold in the current state and we need to replace it.
        // Thus we replace it with a set of literal `x_1 v ... v x_m` such that
        // `x_1 v ... v x_m => literal`

        self.add_implying_literals_to_explanation(literal, cause, &mut explanation, explainer);
        debug_assert!(explanation.lits.iter().copied().all(|l| self.entails(l)));

        // explanation = `!literal v x_1 v ... v x_m`, where all disjuncts hold in the current state
        // we then transform this clause to be in the first unique implication point (1UIP) form.

        self.refine_explanation(explanation, explainer)
    }

    /// Refines an explanation into an asserting clause.
    ///
    /// Note that a partial backtrack (within the current decision level) will occur in the process.
    /// This is necessary to provide explainers with the exact state in which their decisions were made.
    pub fn refine_explanation(&mut self, explanation: Explanation, explainer: &mut impl Explainer) -> Disjunction {
        debug_assert!(explanation.literals().iter().all(|&l| self.entails(l)));
        let mut explanation = explanation;

        // literals falsified at the current decision level, we need to proceed until there is a single one left (1UIP)
        self.queue.clear();
        // literals that are beyond the current decision level and will be part of the final clause
        let mut result: Vec<Lit> = Vec::with_capacity(4);

        let decision_level = self.domains.current_decision_level();

        loop {
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
                            self.queue.push(InQueueLit { cause: loc, lit: l })
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
                return Disjunction::new(result);
            }
            debug_assert!(!self.queue.is_empty());

            // not reached the first UIP yet,
            // select latest falsified literal from queue
            let mut l = self.queue.pop().unwrap();
            // The queue might contain more than one reference to the same event.
            // Due to the priority of the queue, they necessarily contiguous
            while let Some(next) = self.queue.peek() {
                // check if next event is the same one
                if next.cause == l.cause {
                    // they are the same, pop it from the queue
                    let l2 = self.queue.pop().unwrap();
                    // of the two literal, keep the most general one
                    if l2.lit.entails(l.lit) {
                        l = l2;
                    } else {
                        // l is more general, keep it an continue
                        assert!(l.lit.entails(l2.lit));
                    }
                } else {
                    // next is on a different event, we can proceed
                    break;
                }
            }

            if self.queue.is_empty() {
                // We have reached the first Unique Implication Point (UIP)
                // the content of result is a conjunction of literal that imply `!l`
                // build the conflict clause and exit
                debug_assert!(self.queue.is_empty());
                result.push(!l.lit);
                return Disjunction::new(result);
            }

            debug_assert!(l.cause < self.domains.trail().next_slot());
            debug_assert!(self.entails(l.lit));
            let mut cause = None;
            // backtrack until the latest falsifying event
            // this will undo some of the changes but will keep us in the same decision level
            while l.cause < self.domains.trail().next_slot() {
                // the event cannot be a decision, because it would have been detected as a UIP earlier
                debug_assert_ne!(self.domains.last_event().unwrap().cause, Origin::DECISION);
                let x = self.domains.undo_last_event();
                cause = Some(x);
            }
            let cause = cause.unwrap();

            // in the explanation, add a set of literal whose conjunction implies `l.lit`
            self.add_implying_literals_to_explanation(l.lit, cause, &mut explanation, explainer);
        }
    }

    /// Computes literals `l_1 ... l_n` such that:
    ///  - `l_1 & ... & l_n => literal`
    ///  - each `l_i` is entailed at the current level.
    ///
    /// Assumptions:
    ///  - `literal` is not entailed in the current
    ///  - `cause` provides the explanation for asserting `literal` (and is not a decision).
    fn add_implying_literals_to_explanation(
        &mut self,
        literal: Lit,
        cause: Origin,
        explanation: &mut Explanation,
        explainer: &mut impl Explainer,
    ) {
        // we should be in a state where the literal is not true yet, but immediately implied
        debug_assert!(!self.entails(literal));
        match cause {
            Origin::Direct(DirectOrigin::Decision) => panic!(),
            Origin::Direct(DirectOrigin::ExternalInference(cause)) => {
                // ask for a clause (l1 & l2 & ... & ln) => lit
                explainer.explain(cause, literal, self, explanation);
            }
            Origin::Direct(DirectOrigin::ImplicationPropagation(causing_literal)) => explanation.push(causing_literal),
            Origin::PresenceOfEmptyDomain(invalid_lit, cause) => {
                // invalid_lit & !invalid_lit => absent(variable(invalid_lit))
                debug_assert!(self.entails(!invalid_lit));
                explanation.push(!invalid_lit);
                match cause {
                    DirectOrigin::Decision => panic!(),
                    DirectOrigin::ExternalInference(cause) => {
                        // ask for a clause (l1 & l2 & ... & ln) => lit
                        explainer.explain(cause, invalid_lit, self, explanation);
                    }
                    DirectOrigin::ImplicationPropagation(causing_literal) => {
                        explanation.push(causing_literal);
                    }
                }
            }
        }
    }

    pub fn entails(&self, lit: Lit) -> bool {
        self.domains.entails(lit)
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

    pub fn or_value(&self, disjunction: &[Lit]) -> Option<bool> {
        let mut found_undef = false;
        for &disjunct in disjunction {
            match self.value(disjunct) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => found_undef = true,
            }
        }
        if found_undef {
            None
        } else {
            Some(false)
        }
    }

    pub fn bound_variables(&self) -> impl Iterator<Item = (VarRef, IntCst)> + '_ {
        self.domains.bound_variables()
    }

    // ================ Events ===============

    pub fn entailing_level(&self, lit: Lit) -> DecLvl {
        debug_assert!(self.entails(lit));
        match self.implying_event(lit) {
            Some(loc) => self.trail().decision_level(loc),
            None => DecLvl::ROOT,
        }
    }

    pub fn implying_event(&self, lit: Lit) -> Option<EventIndex> {
        self.domains.implying_event(lit)
    }

    pub fn num_events(&self) -> u32 {
        self.domains.num_events()
    }

    pub fn trail(&self) -> &ObsTrail<Event> {
        self.domains.trail()
    }

    pub fn get_event(&self, loc: EventIndex) -> &Event {
        self.domains.trail().get_event(loc)
    }

    // ============== Utils ==============

    pub fn print(&self) {
        for v in self.domains.variables() {
            println!(
                "{:?}\t{}: {:?}",
                v,
                self.label(v).unwrap_or("???"),
                self.domains.bounds(v)
            );
        }
    }

    pub fn fmt(&self, variable: impl Into<VarRef>) -> String {
        let variable = variable.into();
        match self.labels[variable].get() {
            Some(s) => s.to_string(),
            None => format!("{:?}", variable),
        }
    }

    pub fn fmt_lit(&self, lit: Lit) -> String {
        format!("{} {} {}", self.fmt(lit.variable()), lit.relation(), lit.value())
    }
}

impl Default for DiscreteModel {
    fn default() -> Self {
        Self::new()
    }
}

impl Backtrack for DiscreteModel {
    fn save_state(&mut self) -> DecLvl {
        self.domains.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.domains.num_saved()
    }

    fn restore_last(&mut self) {
        self.domains.restore_last()
    }
}

/// A literal in an explanation queue
#[derive(Copy, Clone, Debug)]
struct InQueueLit {
    cause: EventIndex,
    lit: Lit,
}
impl PartialEq for InQueueLit {
    fn eq(&self, other: &Self) -> bool {
        self.cause == other.cause
    }
}
impl Eq for InQueueLit {}
impl Ord for InQueueLit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cause.cmp(&other.cause)
    }
}
impl PartialOrd for InQueueLit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use crate::assignments::{Assignment, OptDomain};
    use crate::bounds::{Lit as ILit, Lit};
    use crate::int_model::cause::Origin;
    use crate::int_model::explanation::{Explainer, Explanation};
    use crate::int_model::{Cause, DiscreteModel, InferenceCause, InvalidUpdate};
    use crate::lang::{BVar, IVar};
    use crate::{Model, WriterId};
    use aries_backtrack::Backtrack;
    use std::collections::HashSet;

    #[test]
    fn domain_updates() {
        let mut model = Model::new();
        let a = model.new_ivar(0, 10, "a");

        assert_eq!(model.discrete.set_lb(a, -1, Cause::Decision), Ok(false));
        assert_eq!(model.discrete.set_lb(a, 0, Cause::Decision), Ok(false));
        assert_eq!(model.discrete.set_lb(a, 1, Cause::Decision), Ok(true));
        assert_eq!(model.discrete.set_ub(a, 11, Cause::Decision), Ok(false));
        assert_eq!(model.discrete.set_ub(a, 10, Cause::Decision), Ok(false));
        assert_eq!(model.discrete.set_ub(a, 9, Cause::Decision), Ok(true));
        // domain is [1, 9]
        assert_eq!(model.domain_of(a), (1, 9));

        model.save_state();
        assert_eq!(model.discrete.set_lb(a, 9, Cause::Decision), Ok(true));
        assert_eq!(
            model.discrete.set_lb(a, 10, Cause::Decision),
            Err(InvalidUpdate(Lit::geq(a, 10), Origin::DECISION))
        );

        model.restore_last();
        assert_eq!(model.domain_of(a), (1, 9));
        assert_eq!(model.discrete.set_ub(a, 1, Cause::Decision), Ok(true));
        assert_eq!(
            model.discrete.set_ub(a, 0, Cause::Decision),
            Err(InvalidUpdate(Lit::leq(a, 0), Origin::DECISION))
        );
    }

    #[test]
    fn test_explanation() {
        let mut model = Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");
        let n = model.new_ivar(0, 10, "n");

        // constraint 0: "a => (n <= 4)"
        // constraint 1: "b => (n >= 5)"

        let writer = WriterId::new(1);

        let cause_a = Cause::inference(writer, 0u32);
        let cause_b = Cause::inference(writer, 1u32);

        #[allow(unused_must_use)]
        let propagate = |model: &mut Model| -> Result<bool, InvalidUpdate> {
            if model.boolean_value_of(a) == Some(true) {
                model.discrete.set_ub(n, 4, cause_a)?;
            }
            if model.boolean_value_of(b) == Some(true) {
                model.discrete.set_lb(n, 5, cause_b)?;
            }
            Ok(true)
        };

        struct Expl {
            a: BVar,
            b: BVar,
            n: IVar,
        }
        impl Explainer for Expl {
            fn explain(
                &mut self,
                cause: InferenceCause,
                literal: ILit,
                _model: &DiscreteModel,
                explanation: &mut Explanation,
            ) {
                assert_eq!(cause.writer, WriterId::new(1));
                match cause.payload {
                    0 => {
                        assert_eq!(literal, ILit::leq(self.n, 4));
                        explanation.push(ILit::is_true(self.a));
                    }
                    1 => {
                        assert_eq!(literal, ILit::geq(self.n, 5));
                        explanation.push(ILit::is_true(self.b));
                    }
                    _ => panic!("unexpected payload"),
                }
            }
        }

        let mut network = Expl { a, b, n };

        propagate(&mut model).unwrap();
        model.save_state();
        model.discrete.set_lb(a, 1, Cause::Decision).unwrap();
        assert_eq!(model.bounds(a.into()), (1, 1));
        propagate(&mut model).unwrap();
        assert_eq!(model.opt_domain_of(n), OptDomain::Present(0, 4));
        model.save_state();
        model.discrete.set_lb(n, 1, Cause::Decision).unwrap();
        model.save_state();
        model.discrete.set_lb(b, 1, Cause::Decision).unwrap();
        let err = match propagate(&mut model) {
            Err(err) => err,
            _ => panic!(),
        };

        let clause = model.discrete.clause_for_invalid_update(err, &mut network);
        let clause: HashSet<_> = clause.literals().iter().copied().collect();

        // we have three rules
        //  -  !(n <= 4) || !(n >= 5)   (conflict)
        //  -  !a || (n <= 4)           (clause a)
        //  -  !b || (n >= 5)           (clause b)
        // Explanation should perform resolution of the first and last rules for the literal (n >= 5):
        //   !(n <= 4) || !b
        //   !b || (n > 4)      (equivalent to previous)
        let mut expected = HashSet::new();
        expected.insert(ILit::is_false(b));
        expected.insert(ILit::gt(n, 4));
        assert_eq!(clause, expected);
    }

    struct NoExplain;
    impl Explainer for NoExplain {
        fn explain(&mut self, _: InferenceCause, _: Lit, _: &DiscreteModel, _: &mut Explanation) {
            panic!("No external cause expected")
        }
    }

    #[test]
    fn test_optional_propagation_error() {
        let mut model = DiscreteModel::new();
        let p = model.new_var(0, 1, "p");
        let i = model.new_optional_var(0, 10, p.geq(1), "i");
        let x = model.new_var(0, 10, "x");

        model.save_state();
        assert_eq!(model.set_lb(p, 1, Cause::Decision), Ok(true));
        model.save_state();
        assert_eq!(model.set_ub(i, 5, Cause::Decision), Ok(true));

        // irrelevant event
        model.save_state();
        assert_eq!(model.set_ub(x, 5, Cause::Decision), Ok(true));

        model.save_state();
        assert!(matches!(model.set_lb(i, 6, Cause::Decision), Err(_)));
    }
}
