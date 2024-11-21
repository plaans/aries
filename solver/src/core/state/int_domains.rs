use crate::backtrack::{Backtrack, BacktrackWith, DecLvl, EventIndex, ObsTrail};
use crate::collections::ref_store::RefVec;
use crate::core::state::cause::Origin;
use crate::core::state::event::{ChangeIndex, Event};
use crate::core::state::InvalidUpdate;
use crate::core::*;

/// Represents a the value of an upper/lower bound of a particular variable.
/// It is packed with the index of the event that caused this change.
///
/// We enforce an alignment on 8 bytes to make sure it can be read and written in a single instruction.
#[derive(Copy, Clone, Debug)]
#[repr(align(8))]
pub struct ValueCause {
    /// Current value of the variable bound.
    pub value: UpperBound,
    /// Index of the event that caused the current value.
    pub cause: ChangeIndex,
}
impl ValueCause {
    pub fn new(value: UpperBound, cause: ChangeIndex) -> Self {
        ValueCause { value, cause }
    }
}

/// Associates every variable to the literals of its domain.
/// In addition, it maintains the history of changes that caused the literals to be in this state,
/// which enables explanations and backtracking.
///
/// **Invariant:** every domain is non empty. Hence any update that would result in an empty domain
/// would return an `Error<EmptyDomain>`.
#[derive(Clone)]
pub struct IntDomains {
    /// Associates each lb/ub of each variable to its current value and the event that caused the latest update.
    bounds: RefVec<SignedVar, ValueCause>,
    /// All events that updated the bound values.
    /// Used for explanation and backtracking.
    events: ObsTrail<Event>,
}

impl IntDomains {
    pub fn new() -> Self {
        let mut uninitialized = IntDomains {
            bounds: Default::default(),
            events: Default::default(),
        };
        let zero = uninitialized.new_var(0, 0);
        let one = uninitialized.new_var(1, 1);
        debug_assert_eq!(zero, VarRef::ZERO);
        debug_assert_eq!(one, VarRef::ONE);
        debug_assert!(uninitialized.entails(Lit::TRUE));
        debug_assert!(!uninitialized.entails(Lit::FALSE));
        uninitialized
    }

    pub fn new_var(&mut self, lb: IntCst, ub: IntCst) -> VarRef {
        let var_lb = self.bounds.push(ValueCause::new(UpperBound::lb(lb), None));
        let var_ub = self.bounds.push(ValueCause::new(UpperBound::ub(ub), None));
        debug_assert_eq!(var_lb.variable(), var_ub.variable());
        debug_assert!(var_lb.is_minus());
        debug_assert!(var_ub.is_plus());
        let var = var_lb.variable();
        self.events.push(Event::initial_upper_bound(var, ub));
        self.events.push(Event::initial_lower_bound(var, lb));
        var
    }

    pub fn ub(&self, var: impl Into<SignedVar>) -> IntCst {
        self.bounds[var.into()].value.as_int()
    }

    pub fn lb(&self, var: impl Into<SignedVar>) -> IntCst {
        // var <= ub   <=>   -var >= -ub
        -self.ub(-var.into())
    }

    pub fn entails(&self, lit: Lit) -> bool {
        self.get_bound_value(lit.svar()).stronger(lit.bound_value())
    }

    #[inline]
    pub fn get_bound_value(&self, var_bound: SignedVar) -> UpperBound {
        self.bounds[var_bound].value
    }

    /// Attempts to set the bound to the given value.
    /// Results:
    ///  - Ok(true): The model was updated ans is consistent.
    ///  - Ok(false): The change is as no-op (was previously entailed) and nothing changed. The model is consistent.
    ///  - Err(EmptyDom(var)): update was not carried out as it would have resulted in an empty domain.
    #[allow(clippy::if_same_then_else)]
    pub fn set_bound(&mut self, affected: SignedVar, new: UpperBound, cause: Origin) -> Result<bool, InvalidUpdate> {
        let current = self.bounds[affected];

        let lit = Lit::from_parts(affected, new);

        if current.value.stronger(new) {
            Ok(false)
        } else {
            let other = self.bounds[affected.neg()].value;
            if new.compatible_with_symmetric(other) {
                self.bounds[affected] = ValueCause::new(new, Some(self.events.next_slot()));
                let event = Event {
                    affected_bound: affected,
                    cause,
                    new_value: new,
                    previous: current,
                };
                // println!("UPDATE: {lit:?} {cause:?}");
                self.events.push(event);
                // update occurred and is consistent
                Ok(true)
            } else {
                // println!("INVALID UPDATE: {lit:?} {cause:?}");
                Err(InvalidUpdate(lit, cause))
            }
        }
    }

    // ============= Variables =================

    /// Returns the number of variables declared.
    pub fn num_variables(&self) -> usize {
        debug_assert!(self.bounds.len() % 2 == 0);
        self.bounds.len() / 2
    }

    /// Returns all variables.
    pub fn variables(&self) -> impl Iterator<Item = VarRef> {
        (0..self.num_variables()).map(VarRef::from)
    }

    /// Returns all variables whose value is fixed.
    pub fn bound_variables(&self) -> impl Iterator<Item = (VarRef, IntCst)> + '_ {
        self.variables().filter_map(move |v| {
            let lb = self.lb(v);
            let ub = self.ub(v);
            if lb == ub {
                Some((v, lb))
            } else {
                None
            }
        })
    }

    // =========== History ===================

    /// Returns the index of the first event that makes `lit` true.
    /// If the function returns None, it means that `lit` was true at the root level.
    pub fn implying_event(&self, lit: Lit) -> Option<EventIndex> {
        debug_assert!(self.entails(lit));
        let mut cur = self.bounds[lit.svar()].cause;
        while let Some(loc) = cur {
            let ev = self.events.get_event(loc);
            if ev.makes_true(lit) {
                break;
            } else {
                cur = ev.previous.cause
            }
        }
        cur
    }

    /// Returns the list of upper bounds that have been set to this variable from most recent (strongest)
    /// to the initial value. Each upper-bound is tagged with the index of the event that enforced it.
    ///
    /// A typical output would a stream:
    ///  - (ub: 10, Some(event-id-: 14)    current upper bound is 10 and was enforced by the 14th event
    ///  - (ub: 16, Some(event-id-: 11)    previous upper bound was 16, enforced by the 11th even
    ///  - (ub: 20, None)                  Initial value of the upper bound was 20
    pub(super) fn upper_bounds_history(
        &self,
        var: SignedVar,
    ) -> impl Iterator<Item = (IntCst, Option<EventIndex>)> + '_ {
        enum Next {
            Event(EventIndex),
            Value(IntCst),
            None,
        }
        struct Iter<'a> {
            next: Next,
            doms: &'a IntDomains,
        }
        impl<'a> Iterator for Iter<'a> {
            type Item = (IntCst, Option<EventIndex>);

            fn next(&mut self) -> Option<Self::Item> {
                match self.next {
                    Next::Event(loc) => {
                        let ev = self.doms.events.get_event(loc);
                        if let Some(previous_index) = ev.previous.cause {
                            self.next = Next::Event(previous_index)
                        } else {
                            self.next = Next::Value(ev.previous.value.as_int())
                        }
                        Some((ev.new_value.as_int(), Some(loc)))
                    }
                    Next::Value(value) => {
                        self.next = Next::None;
                        Some((value, None))
                    }
                    Next::None => None,
                }
            }
        }
        let next = if let Some(event_index) = self.bounds[var].cause {
            Next::Event(event_index)
        } else {
            // initial value
            Next::Value(self.ub(var))
        };
        Iter { next, doms: self }
    }

    pub fn num_events(&self) -> u32 {
        self.events.num_events()
    }

    pub fn last_event(&self) -> Option<&Event> {
        self.events.peek()
    }

    pub fn trail(&self) -> &ObsTrail<Event> {
        &self.events
    }

    // =============== State management ===================

    fn undo_event(bounds: &mut RefVec<SignedVar, ValueCause>, ev: &Event) {
        bounds[ev.affected_bound] = ev.previous;
    }

    pub fn undo_last_event(&mut self) -> Origin {
        let ev = self.events.pop().unwrap();
        let bounds = &mut self.bounds;
        Self::undo_event(bounds, &ev);
        ev.cause
    }
}

impl Default for IntDomains {
    fn default() -> Self {
        Self::new()
    }
}

impl Backtrack for IntDomains {
    fn save_state(&mut self) -> DecLvl {
        self.events.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.events.num_saved()
    }

    fn restore_last(&mut self) {
        let bounds = &mut self.bounds;
        self.events.restore_last_with(|ev| {
            Self::undo_event(bounds, ev);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entails() {
        let mut m = IntDomains::default();
        let a = m.new_var(0, 10);
        assert_eq!(m.lb(a), 0);
        assert_eq!(m.ub(a), 10);
        assert!(m.entails(a.geq(-2)));
        assert!(m.entails(a.geq(-1)));
        assert!(m.entails(a.geq(0)));
        assert!(!m.entails(a.geq(1)));
        assert!(!m.entails(a.geq(2)));
        assert!(!m.entails(a.geq(10)));

        assert!(m.entails(a.leq(12)));
        assert!(m.entails(a.leq(11)));
        assert!(m.entails(a.leq(10)));
        assert!(!m.entails(a.leq(9)));
        assert!(!m.entails(a.leq(8)));
        assert!(!m.entails(a.leq(0)));
    }

    #[test]
    fn test_variable_iter() {
        let mut m = IntDomains::default();
        let a = m.new_var(0, 10);
        let b = m.new_var(1, 1);
        let c = m.new_var(3, 7);

        let vars: Vec<VarRef> = m.variables().collect();
        assert_eq!(vars, vec![VarRef::ZERO, VarRef::ONE, a, b, c]);
    }
}
