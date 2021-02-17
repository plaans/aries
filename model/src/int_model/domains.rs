use crate::bounds::{Bound, BoundValue, VarBound};
use crate::int_model::{Cause, EmptyDomain};
use crate::lang::{IntCst, VarRef};
use aries_backtrack::{Backtrack, BacktrackWith, DecLvl, EventIndex, ObsTrail};
use aries_collections::ref_store::RefVec;
use std::fmt::{Debug, Formatter};

type ChangeIndex = Option<EventIndex>;

#[derive(Clone)]
pub struct Event {
    pub affected_bound: VarBound,
    pub cause: Cause,
    pub previous_value: BoundValue,
    pub new_value: BoundValue,
    pub previous_event: ChangeIndex,
}

impl Event {
    #[inline]
    pub fn makes_true(&self, lit: Bound) -> bool {
        debug_assert_eq!(self.affected_bound, lit.affected_bound());
        self.new_value.stronger(lit.bound_value()) && !self.previous_value.stronger(lit.bound_value())
    }

    #[inline]
    pub fn new_literal(&self) -> Bound {
        Bound::from_parts(self.affected_bound, self.new_value)
    }
}

impl Debug for Event {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} \tprev: {:?} \tcaused_by: {:?}",
            self.affected_bound.bind(self.new_value),
            self.affected_bound.bind(self.previous_value),
            self.cause
        )
    }
}

#[derive(Default, Clone)]
pub struct Domains {
    bounds: RefVec<VarBound, (BoundValue, ChangeIndex)>,
    events: ObsTrail<Event>,
}

impl Domains {
    pub fn new_var(&mut self, lb: IntCst, ub: IntCst) -> VarRef {
        let var_lb = self.bounds.push((BoundValue::lb(lb), None));
        let var_ub = self.bounds.push((BoundValue::ub(ub), None));
        // self.causes_index.push(None);
        // self.causes_index.push(None);
        debug_assert_eq!(var_lb.variable(), var_ub.variable());
        debug_assert!(var_lb.is_lb());
        debug_assert!(var_ub.is_ub());
        var_lb.variable()
    }

    // ============== Accessors =====================

    pub fn bounds(&self, v: VarRef) -> (IntCst, IntCst) {
        (self.lb(v), self.ub(v))
    }

    pub fn ub(&self, var: VarRef) -> IntCst {
        self.bounds[VarBound::ub(var)].0.as_ub()
    }

    pub fn lb(&self, var: VarRef) -> IntCst {
        self.bounds[VarBound::lb(var)].0.as_lb()
    }

    pub fn entails(&self, lit: Bound) -> bool {
        self.bounds[lit.affected_bound()].0.stronger(lit.bound_value())
    }

    #[inline]
    pub fn get_bound(&self, var_bound: VarBound) -> BoundValue {
        self.bounds[var_bound].0
    }

    // ============== Updates ==============

    #[inline]
    pub fn set_lb(&mut self, var: VarRef, new_lb: IntCst, cause: Cause) -> Result<bool, EmptyDomain> {
        self.set_bound(VarBound::lb(var), BoundValue::lb(new_lb), cause)
    }

    #[inline]
    pub fn set_ub(&mut self, var: VarRef, new_ub: IntCst, cause: Cause) -> Result<bool, EmptyDomain> {
        self.set_bound(VarBound::ub(var), BoundValue::ub(new_ub), cause)
    }

    #[inline]
    pub fn set(&mut self, literal: Bound, cause: Cause) -> Result<bool, EmptyDomain> {
        self.set_bound(literal.affected_bound(), literal.bound_value(), cause)
    }

    pub fn set_bound(&mut self, affected: VarBound, new: BoundValue, cause: Cause) -> Result<bool, EmptyDomain> {
        let entry = self.bounds[affected];
        let prev = entry.0;
        if prev.stronger(new) {
            Ok(false)
        } else {
            self.bounds[affected] = (new, Some(self.events.next_slot()));
            let event = Event {
                affected_bound: affected,
                cause,
                previous_value: prev,
                new_value: new,
                previous_event: entry.1,
            };
            self.events.push(event);

            let other = self.bounds[affected.symmetric_bound()].0;
            if new.compatible_with_symmetric(other) {
                Ok(true)
            } else {
                Err(EmptyDomain(affected.variable()))
            }
        }
    }

    #[inline]
    pub fn set_unchecked(&mut self, literal: Bound, cause: Cause) {
        self.set_bound_unchecked(literal.affected_bound(), literal.bound_value(), cause)
    }

    pub fn set_bound_unchecked(&mut self, affected: VarBound, new: BoundValue, cause: Cause) {
        debug_assert!(new.strictly_stronger(self.bounds[affected].0));
        debug_assert!(new.compatible_with_symmetric(self.bounds[affected.symmetric_bound()].0));
        let prev = self.bounds[affected];
        let next = (new, Some(self.events.next_slot()));
        self.bounds[affected] = next;
        let event = Event {
            affected_bound: affected,
            cause,
            previous_value: prev.0,
            new_value: new,
            previous_event: prev.1,
        };
        self.events.push(event);
    }

    // ============= Variables =================

    pub fn variables(&self) -> impl Iterator<Item = VarRef> {
        (0..self.bounds.len()).step_by(2).map(|b| VarRef::from(b as u32 >> 1))
    }

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

    // history

    pub fn implying_event(&self, lit: Bound) -> Option<EventIndex> {
        let mut cur = self.bounds[lit.affected_bound()].1;
        while let Some(loc) = cur {
            let ev = self.events.get_event(loc);
            if ev.makes_true(lit) {
                break;
            } else {
                cur = ev.previous_event
            }
        }
        cur
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

    // State management

    fn undo_event(bounds: &mut RefVec<VarBound, (BoundValue, ChangeIndex)>, ev: &Event) {
        let entry = &mut bounds[ev.affected_bound];
        entry.0 = ev.previous_value;
        entry.1 = ev.previous_event;
    }

    pub fn undo_last_event(&mut self) -> Cause {
        let ev = self.events.pop().unwrap();
        let bounds = &mut self.bounds;
        Self::undo_event(bounds, &ev);
        ev.cause
    }
}

impl Backtrack for Domains {
    fn save_state(&mut self) -> DecLvl {
        self.events.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.events.num_saved()
    }

    fn restore_last(&mut self) {
        let bounds = &mut self.bounds;
        self.events.restore_last_with(|ev| {
            Self::undo_event(bounds, &ev);
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::int_model::domains::Domains;

    #[test]
    fn test_entails() {
        let mut m = Domains::default();
        let a = m.new_var(0, 10);
        assert_eq!(m.bounds(a), (0, 10));
        assert!(m.entails(a.geq(-2)));
        assert!(m.entails(a.geq(-1)));
        assert!(m.entails(a.geq(0)));
        assert!(!m.entails(a.geq(1)));
        assert!(!m.entails(a.geq(2)));
        assert!(!m.entails(a.geq(10)));

        assert_eq!(m.bounds(a), (0, 10));
        assert!(m.entails(a.leq(12)));
        assert!(m.entails(a.leq(11)));
        assert!(m.entails(a.leq(10)));
        assert!(!m.entails(a.leq(9)));
        assert!(!m.entails(a.leq(8)));
        assert!(!m.entails(a.leq(0)));
    }
}
