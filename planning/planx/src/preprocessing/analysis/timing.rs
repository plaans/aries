use crate::*;

use super::{Bounds, NumericBounds};

/// Allows comparing timestamps appearing in a given action, based on the bounds of its duration (see [`NumericBounds`]).
///
/// All comparisons are *provable* relations: when a method returns `false`, the relation may still hold
/// for some instances of the action.
#[derive(Copy, Clone, Debug)]
pub struct ActionTiming {
    /// Lower and upper bounds on the duration of the action.
    duration: Bounds,
}

impl ActionTiming {
    pub fn new(action: &Action, bounds: &NumericBounds) -> Self {
        let duration = match action.duration {
            Duration::Instantaneous => Bounds::singleton(RealValue::ZERO),
            Duration::Fixed(d) => bounds.bounds(d),
            Duration::Bounded(lb, ub) => Bounds {
                lb: bounds.bounds(lb).lb,
                ub: bounds.bounds(ub).ub,
            },
            Duration::Subtasks => Bounds::UNKNOWN,
        };
        Self { duration }
    }

    /// Returns lower and upper bounds on the delay `t2 - t1`. A bound is `None` if unknown.
    pub fn delay_bounds(&self, t1: Timestamp, t2: Timestamp) -> (Option<RealValue>, Option<RealValue>) {
        let offset = t2.delay - t1.delay;
        let Bounds { lb, ub } = self.duration;
        match (t1.reference, t2.reference) {
            (r1, r2) if r1 == r2 => (Some(offset), Some(offset)),
            // t2 - t1 = duration + offset
            (TimeRef::ActionStart, TimeRef::ActionEnd) => (lb.map(|d| d + offset), ub.map(|d| d + offset)),
            // t2 - t1 = -duration + offset
            (TimeRef::ActionEnd, TimeRef::ActionStart) => (ub.map(|d| offset - d), lb.map(|d| offset - d)),
            _ => (None, None),
        }
    }

    /// True if `t1 <= t2` in any instance of the action.
    pub fn le(&self, t1: Timestamp, t2: Timestamp) -> bool {
        matches!(self.delay_bounds(t1, t2).0, Some(lb) if lb >= RealValue::ZERO)
    }

    /// True if `t1 < t2` in any instance of the action.
    pub fn lt(&self, t1: Timestamp, t2: Timestamp) -> bool {
        matches!(self.delay_bounds(t1, t2).0, Some(lb) if lb > RealValue::ZERO)
    }

    /// True if `t1 = t2` in any instance of the action.
    pub fn eq(&self, t1: Timestamp, t2: Timestamp) -> bool {
        self.le(t1, t2) && self.le(t2, t1)
    }

    /// True if a condition over the interval guarantees that it holds in the state just before `t`
    /// (i.e. before the effects occurring at `t`), in any instance of the action.
    ///
    /// This is the case if `t` is in the interval, or if `t` is the upper bound of an open interval:
    /// as the condition holds until `t`, it can only be changed by an effect at `t`.
    pub fn holds_before(&self, itv: TimeInterval, t: Timestamp) -> bool {
        match itv {
            TimeInterval::Closed { start, end } => self.le(start, t) && self.le(t, end),
            TimeInterval::Open { start, end } => self.lt(start, t) && self.le(t, end),
        }
    }

    /// True if, in any instance of the action, the two conditions both hold just before one of their bounds.
    pub fn hold_together(&self, a: TimeInterval, b: TimeInterval) -> bool {
        [bounds(a), bounds(b)]
            .into_iter()
            .flatten()
            .any(|t| self.holds_before(a, t) && self.holds_before(b, t))
    }
}

fn bounds(itv: TimeInterval) -> [Timestamp; 2] {
    match itv {
        TimeInterval::Closed { start, end } | TimeInterval::Open { start, end } => [start, end],
    }
}
