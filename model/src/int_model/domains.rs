use crate::bounds::{Bound, BoundValue, VarBound};
use crate::int_model::{Cause, EmptyDomain};
use crate::lang::{IntCst, VarRef};
use aries_backtrack::{Backtrack, BacktrackWith, ObsTrail, TrailLoc};
use aries_collections::ref_store::RefVec;

type EventIndex = Option<TrailLoc>;

#[derive(Clone, Debug)]
struct Event {
    affected_bound: VarBound,
    cause: Cause,
    previous_value: BoundValue,
    new_value: BoundValue,
    previous_event: EventIndex,
}

impl Event {
    pub fn makes_true(&self, lit: Bound) -> bool {
        debug_assert_eq!(self.affected_bound, lit.affected_bound());
        self.new_value.stronger(lit.bound_value()) && !self.previous_value.stronger(lit.bound_value())
    }
}

#[derive(Default, Clone)]
pub struct Domains {
    bounds: RefVec<VarBound, BoundValue>,
    causes_index: RefVec<VarBound, EventIndex>,
    events: ObsTrail<Event>,
}

impl Domains {
    pub fn new_var(&mut self, lb: IntCst, ub: IntCst) -> VarRef {
        let var_lb = self.bounds.push(BoundValue::new_lb(lb));
        let var_ub = self.bounds.push(BoundValue::new_ub(ub));
        self.causes_index.push(None);
        self.causes_index.push(None);
        debug_assert_eq!(var_lb.variable(), var_ub.variable());
        debug_assert!(var_lb.is_lb());
        debug_assert!(var_ub.is_ub());
        var_lb.variable()
    }

    pub fn bounds(&self, v: VarRef) -> (IntCst, IntCst) {
        (self.lb(v), self.ub(v))
    }

    pub fn ub(&self, var: VarRef) -> IntCst {
        self.bounds[VarBound::ub(var)].as_ub()
    }

    pub fn lb(&self, var: VarRef) -> IntCst {
        self.bounds[VarBound::lb(var)].as_lb()
    }

    pub fn entails(&self, lit: Bound) -> bool {
        self.bounds[lit.affected_bound()].stronger(lit.bound_value())
    }

    pub fn set_lb(&mut self, var: VarRef, new_lb: IntCst, cause: Cause) -> Result<bool, EmptyDomain> {
        let (lb, ub) = self.bounds(var);
        let vb = VarBound::lb(var);
        if new_lb > lb {
            self.bounds[vb] = BoundValue::new_lb(new_lb);
            let previous_event = self.causes_index[vb];
            self.causes_index[vb] = Some(self.events.next_slot());
            let event = Event {
                affected_bound: vb,
                cause,
                previous_value: BoundValue::new_lb(lb),
                new_value: BoundValue::new_lb(new_lb),
                previous_event,
            };
            self.events.push(event);
            if new_lb > ub {
                Err(EmptyDomain(var))
            } else {
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    pub fn set_ub(&mut self, var: VarRef, new_ub: IntCst, cause: Cause) -> Result<bool, EmptyDomain> {
        let (lb, ub) = self.bounds(var);
        let vb = VarBound::ub(var);
        if new_ub < ub {
            self.bounds[vb] = BoundValue::new_ub(new_ub);
            let previous_event = self.causes_index[vb];
            self.causes_index[vb] = Some(self.events.next_slot());
            let event = Event {
                affected_bound: vb,
                cause,
                previous_value: BoundValue::new_ub(ub),
                new_value: BoundValue::new_ub(new_ub),
                previous_event,
            };
            self.events.push(event);
            if lb > new_ub {
                Err(EmptyDomain(var))
            } else {
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

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

    pub fn implying_event(&self, lit: Bound) -> Option<TrailLoc> {
        let mut cur = self.causes_index[lit.affected_bound()];
        while let Some(loc) = cur {
            let ev = &self.events.events()[loc.event_index];
            if ev.makes_true(lit) {
                break;
            } else {
                cur = ev.previous_event
            }
        }
        cur
    }

    // State management

    fn undo_event(
        bounds: &mut RefVec<VarBound, BoundValue>,
        causes_index: &mut RefVec<VarBound, EventIndex>,
        ev: &Event,
    ) {
        bounds[ev.affected_bound] = ev.previous_value;
        causes_index[ev.affected_bound] = ev.previous_event;
    }

    pub fn undo_last_event(&mut self) -> Cause {
        let ev = self.events.pop().unwrap();
        let bounds = &mut self.bounds;
        let causes_index = &mut self.causes_index;
        Self::undo_event(bounds, causes_index, &ev);
        ev.cause
    }
}

impl Backtrack for Domains {
    fn save_state(&mut self) -> u32 {
        self.events.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.events.num_saved()
    }

    fn restore_last(&mut self) {
        let bounds = &mut self.bounds;
        let causes_index = &mut self.causes_index;
        self.events.restore_last_with(|ev| {
            Self::undo_event(bounds, causes_index, &ev);
        })
    }
}
