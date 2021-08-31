use crate::bounds::{BoundValue, Lit, VarBound};
use crate::int_model::cause::{DirectOrigin, Origin};
use crate::int_model::event::Event;
use crate::int_model::int_domains::IntDomains;
use crate::int_model::presence_graph::TwoSatTree;
use crate::int_model::{Cause, InvalidUpdate};
use crate::lang::{IntCst, VarRef};
use aries_backtrack::{Backtrack, DecLvl, EventIndex, ObsTrail};
use aries_collections::ref_store::RefMap;

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
pub struct OptDomains {
    /// Integer part of the domains.
    doms: IntDomains,
    /// If a variable is optional, associates it with a literal that
    /// is true if and only if the variable is present.
    presence: RefMap<VarRef, Lit>,
    /// A graph to encode the relations between presence variables.
    presence_graph: TwoSatTree,
}

impl OptDomains {
    pub fn new() -> Self {
        let domains = OptDomains {
            doms: IntDomains::new(),
            presence: Default::default(),
            presence_graph: Default::default(),
        };
        debug_assert!(domains.entails(Lit::TRUE));
        debug_assert!(!domains.entails(Lit::FALSE));
        domains
    }

    pub fn new_var(&mut self, lb: IntCst, ub: IntCst) -> VarRef {
        self.doms.new_var(lb, ub)
    }

    pub fn new_presence_literal(&mut self, scope: Lit) -> Lit {
        let lit = self.new_var(0, 1).geq(1);
        self.presence_graph.add_implication(lit, scope);
        if self.entails(!scope) {
            let prop_result = self.set_impl(!lit, DirectOrigin::ImplicationPropagation(!scope));
            assert_eq!(prop_result, Ok(true));
        }
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

    pub fn presence(&self, var: VarRef) -> Lit {
        self.presence.get(var).copied().unwrap_or(Lit::TRUE)
    }

    /// Returns `true` if `presence(a) => presence(b)`
    pub fn only_present_with(&self, a: VarRef, b: VarRef) -> bool {
        let prez_a = self.presence(a);
        let prez_b = self.presence(b);
        // prez_a => prez_b
        prez_b == Lit::TRUE || prez_a.entails(prez_b) || self.presence_graph.implies(prez_a, prez_b)
    }

    /// Returns true if we know that two variable are always present jointly.
    pub fn always_present_together(&self, a: VarRef, b: VarRef) -> bool {
        self.presence(a) == self.presence(b)
    }

    /// Returns `true` if the variable is necessarily present and `false` if it is necessarily absent.
    /// Otherwise, the presence status of the variable is unknown and `None` is returned.
    pub fn present(&self, var: VarRef) -> Option<bool> {
        let presence = self.presence(var);
        if self.entails(presence) {
            Some(true)
        } else if self.entails(!presence) {
            Some(false)
        } else {
            None
        }
    }

    // ============== Integer domain accessors =====================

    pub fn bounds(&self, v: VarRef) -> (IntCst, IntCst) {
        (self.lb(v), self.ub(v))
    }

    pub fn ub(&self, var: VarRef) -> IntCst {
        self.doms.ub(var)
    }

    pub fn lb(&self, var: VarRef) -> IntCst {
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

    #[inline]
    pub fn get_bound(&self, var_bound: VarBound) -> BoundValue {
        self.doms.get_bound_value(var_bound)
    }

    // ============== Updates ==============

    #[inline]
    pub fn set_lb(&mut self, var: VarRef, new_lb: IntCst, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.set_bound(VarBound::lb(var), BoundValue::lb(new_lb), cause)
    }

    #[inline]
    pub fn set_ub(&mut self, var: VarRef, new_ub: IntCst, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.set_bound(VarBound::ub(var), BoundValue::ub(new_ub), cause)
    }

    #[inline]
    pub fn set(&mut self, literal: Lit, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.set_bound(literal.affected_bound(), literal.bound_value(), cause)
    }
    #[inline]
    fn set_impl(&mut self, literal: Lit, cause: DirectOrigin) -> Result<bool, InvalidUpdate> {
        self.set_bound_impl(literal.affected_bound(), literal.bound_value(), Origin::Direct(cause))
    }

    pub fn set_bound(&mut self, affected: VarBound, new: BoundValue, cause: Cause) -> Result<bool, InvalidUpdate> {
        self.set_bound_impl(affected, new, cause.into())
    }

    fn set_bound_impl(&mut self, affected: VarBound, new: BoundValue, cause: Origin) -> Result<bool, InvalidUpdate> {
        match self.presence(affected.variable()) {
            Lit::TRUE => self.set_bound_non_optional(affected, new, cause),
            _ => self.set_bound_optional(affected, new, cause),
        }
    }

    fn set_bound_optional(
        &mut self,
        affected: VarBound,
        new: BoundValue,
        cause: Origin,
    ) -> Result<bool, InvalidUpdate> {
        let prez = self.presence(affected.variable());
        // variable must be optional
        debug_assert_ne!(prez, Lit::TRUE);
        // invariant: optional variable cannot be involved in implications
        debug_assert_eq!(
            self.presence_graph
                .direct_implications_of(Lit::from_parts(affected, new))
                .next(),
            None
        );

        let new_bound = Lit::from_parts(affected, new);

        if self.entails(!prez) {
            // variable is absent, we do nothing
            Ok(false)
        } else if !self.doms.entails(!new_bound) {
            // variable is not proven absent and this is a valid update
            let res = self.doms.set_bound(affected, new, cause);
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
            self.set_bound_non_optional(
                not_prez.affected_bound(),
                not_prez.bound_value(),
                Origin::PresenceOfEmptyDomain(new_bound, origin),
            )
        }
    }

    fn set_bound_non_optional(
        &mut self,
        affected: VarBound,
        new: BoundValue,
        cause: Origin,
    ) -> Result<bool, InvalidUpdate> {
        // remember the top of the event stack
        let mut cursor = self.trail().reader();
        cursor.move_to_end(self.trail());

        debug_assert_eq!(self.presence(affected.variable()), Lit::TRUE);

        // variable is necessarily present, perform update
        let res = self.doms.set_bound(affected, new, cause);
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
                    for implied in self.presence_graph.direct_implications_of(lit) {
                        self.doms.set_bound(
                            implied.affected_bound(),
                            implied.bound_value(),
                            Origin::implication_propagation(lit),
                        )?;
                    }
                }
                // we propagated everything without any error, we are good to go
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(InvalidUpdate(lit, fail_cause)) => {
                debug_assert_eq!(lit, Lit::from_parts(affected, new));
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

    pub fn set_bound_unchecked(&mut self, affected: VarBound, new: BoundValue, cause: Cause) {
        // todo: to have optimal performance, we should implement the unchecked version in IntDomains
        let res = self.set_bound(affected, new, cause);
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

    // State management

    pub fn undo_last_event(&mut self) -> Origin {
        self.doms.undo_last_event()
    }
}

impl Default for OptDomains {
    fn default() -> Self {
        Self::new()
    }
}

impl Backtrack for OptDomains {
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

#[cfg(test)]
mod tests {
    use crate::bounds::Lit;
    use crate::int_model::domains::OptDomains;
    use crate::int_model::{Cause, InvalidUpdate};

    #[test]
    fn test_optional() {
        let mut domains = OptDomains::default();
        let p1 = domains.new_presence_literal(Lit::TRUE);
        // p2 is present if p1 is true
        let p2 = domains.new_presence_literal(p1);
        // i is present if p2 is true
        let i = domains.new_optional_var(0, 10, p2);

        let check_doms = |domains: &OptDomains, lp1, up1, lp2, up2, li, ui| {
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
        let mut domains = OptDomains::new();
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
}
