/*!
Adapts the struct [`SumElem`] and [`LbBoundEvent`] from [`crate::reasoners::cp::linear`] to be compatible with LP certificates.
*/

use std::{
    cmp::Ordering,
    fmt::{Debug, Formatter},
};

use crate::prelude::*;
use crate::{
    backtrack::EventIndex,
    core::{
        Lit, SignedVar, Var,
        state::{DomainsSnapshot, Event},
    },
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct SumElem {
    pub factor: i128,
    pub var: SignedVar,
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
    pub fn new(factor: i128, var: Var) -> Self {
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
}

pub(super) struct LbBoundEvent<'a> {
    pub elem: SumElem,
    pub event: EventIndex,
    pub domains: &'a DomainsSnapshot<'a>,
}

impl<'a> Debug for LbBoundEvent<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} : {} <- {}", self.elem, self.lb(), self.previous_lb())
    }
}

impl<'a> LbBoundEvent<'a> {
    pub fn new(elem: SumElem, domains: &'a DomainsSnapshot) -> Option<Self> {
        let var_ub = domains.lb(elem.var);
        let lit = Lit::geq(elem.var, var_ub);
        let event = domains.implying_event(lit)?;

        Some(Self { elem, event, domains })
    }
    pub fn event(&self) -> &Event {
        self.domains.get_event(self.event)
    }

    pub fn literal(&self) -> Lit {
        self.event().new_literal()
    }

    /// Lower bound of the element (accounting for the factor) entailed by this event.
    pub fn lb(&self) -> i128 {
        // since we are looking for a lower bound, the event will be on an upper bound of the negated variable
        debug_assert_eq!(self.elem.var, -self.event().affected_bound);
        let var_lb = -self.event().new_upper_bound as i128;
        var_lb.saturating_mul(self.elem.factor)
    }

    /// Lower bound of the element (accounting for the factor) BEFORE this event.
    pub fn previous_lb(&self) -> i128 {
        // since we are looking for a lower bound, the event will be on an upper bound of the negated variable
        debug_assert_eq!(self.elem.var, -self.event().affected_bound);
        let previous_var_lb = -self.event().previous.upper_bound as i128;
        previous_var_lb.saturating_mul(self.elem.factor)
    }

    /// Returns the previous lower bound event (that preceded this one).
    /// Return `None` if there was no previous event (i.e. the `prev_lb` was entailed at ROOT).
    pub fn into_previous(self) -> Option<Self> {
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
