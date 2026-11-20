use crate::*;

use super::{Bounds, NumericBounds};

/// Allows comparing timestamps appearing in a given action, based on the bounds of its duration (see [`NumericBounds`]).
///
/// All comparisons are *provable* relations: when a method returns `false`, the relation may still hold
/// for some instances of the action.
///
/// The duration `dt` of a time unit is only fixed when the model is converted to integers: comparisons hold for any
/// `dt` in `(0, dt_max]` (see [`NumericBounds::dt_max`]).
#[derive(Clone, Debug)]
pub struct ActionTiming {
    /// Lower and upper bounds on the duration of the action.
    duration: Bounds,
    /// Upper bound on `dt` (`None` if unknown).
    dt_max: Option<RealValue>,
    /// Start of the transitions of the action's effects, or `None` outside of an action.
    effect_starts: Option<std::sync::Arc<[Timestamp]>>,
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
        Self {
            duration,
            dt_max: bounds.dt_max,
            effect_starts: Some(
                (action.effects.iter())
                    .map(|e| e.effect_expression.transition_start)
                    .collect(),
            ),
        }
    }

    /// Allows comparing timestamps outside of any action (e.g. in the problem definition):
    /// only timestamps with the same reference can be compared.
    pub fn global(bounds: &NumericBounds) -> Self {
        Self {
            duration: Bounds::UNKNOWN,
            dt_max: bounds.dt_max,
            effect_starts: None,
        }
    }

    /// Returns a lower bound on the delay `t2 - t1` (`None` if unknown), together with a flag that is true if
    /// the delay is in fact strictly greater than this bound.
    fn delay_lower_bound(&self, t1: Timestamp, t2: Timestamp) -> Option<(RealValue, bool)> {
        let offset = t2.delay - t1.delay;
        // contribution `num_dt * dt` of the time units, for any `dt` in `(0, dt_max]`
        let num_dt = (t2.num_dt - t1.num_dt) as i64;
        let (eps_lb, strict) = match num_dt {
            0 => (RealValue::ZERO, false),
            n if n > 0 => (RealValue::ZERO, true),
            n => (self.dt_max? * n, false),
        };
        let offset = offset + eps_lb;
        let Bounds { lb, ub } = self.duration;
        let lb = match (t1.reference, t2.reference) {
            (r1, r2) if r1 == r2 => Some(offset),
            // t2 - t1 = duration + offset
            (TimeRef::ActionStart, TimeRef::ActionEnd) => lb.map(|d| d + offset),
            // t2 - t1 = -duration + offset
            (TimeRef::ActionEnd, TimeRef::ActionStart) => ub.map(|d| offset - d),
            _ => None,
        };
        lb.map(|lb| (lb, strict))
    }

    /// True if `t1 <= t2` in any instance of the action.
    pub fn le(&self, t1: Timestamp, t2: Timestamp) -> bool {
        matches!(self.delay_lower_bound(t1, t2), Some((lb, _)) if lb >= RealValue::ZERO)
    }

    /// True if `t1 < t2` in any instance of the action.
    pub fn lt(&self, t1: Timestamp, t2: Timestamp) -> bool {
        matches!(self.delay_lower_bound(t1, t2), Some((lb, strict)) if lb > RealValue::ZERO || (strict && lb == RealValue::ZERO))
    }

    /// True if `t1 = t2` in any instance of the action.
    pub fn eq(&self, t1: Timestamp, t2: Timestamp) -> bool {
        self.le(t1, t2) && self.le(t2, t1)
    }

    /// True if a condition over the interval guarantees that it holds in the state just before `t`
    /// (i.e. before the effects occurring at `t`), in any instance of the action.
    ///
    /// This is the case if `t` is in the interval, or if `t` immediately follows the interval (e.g. `t` is the upper
    /// bound of an open interval `]s, t[`, represented as `[s + dt, t - dt]`), provided no effect of the action may
    /// start in `[end, t)`: the value at `end` can then only be changed by an effect of another action starting at
    /// `end`, which is forbidden by the PDDL mutex (a condition cannot meet the transition of another action's effect).
    pub fn holds_before(&self, itv: TimeInterval, t: Timestamp) -> bool {
        self.le(itv.start, t) && (self.le(t, itv.end) || self.immediately_follows(itv.end, t))
    }

    /// True if `t` is at most one time unit after `end` and no effect of the action may start in `[end, t)`.
    /// Always false outside of an action, where the effects are unknown.
    fn immediately_follows(&self, end: Timestamp, t: Timestamp) -> bool {
        let Some(effect_starts) = &self.effect_starts else {
            return false;
        };
        self.le(t, end + Dt)
            && effect_starts
                .iter()
                .all(|&start| self.lt(start, end) || self.le(t, start))
    }

    /// True if, in any instance of the action, the two conditions both hold just before one of their bounds.
    pub fn hold_together(&self, a: TimeInterval, b: TimeInterval) -> bool {
        [a.start, a.end, b.start, b.end]
            .into_iter()
            .any(|t| self.holds_before(a, t) && self.holds_before(b, t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start() -> Timestamp {
        Timestamp::from(TimeRef::ActionStart)
    }
    fn end() -> Timestamp {
        Timestamp::from(TimeRef::ActionEnd)
    }

    /// Timing of an action with the given duration and effects starting at the given timestamps.
    fn timing(duration: RealValue, dt_max: Option<RealValue>, effect_starts: &[Timestamp]) -> ActionTiming {
        ActionTiming {
            duration: Bounds::singleton(duration),
            dt_max,
            effect_starts: Some(effect_starts.into()),
        }
    }

    #[test]
    fn comparisons_hold_for_any_dt() {
        let quarter = RealValue::new(1, 4);
        // the over all interval `[start + dt, end - dt]` of an action of duration 1/4 is non-empty iff dt <= 1/8
        let itv = TimeInterval::open(start(), end());
        assert!(timing(quarter, Some(RealValue::new(1, 8)), &[]).le(itv.start, itv.end));
        assert!(!timing(quarter, Some(quarter), &[]).le(itv.start, itv.end));
        assert!(!timing(quarter, None, &[]).le(itv.start, itv.end));
        // a single time unit is positive, whatever its duration
        assert!(timing(quarter, None, &[]).lt(start(), start() + Dt));
        assert!(!timing(quarter, None, &[]).lt(start(), start()));
        assert!(timing(quarter, Some(quarter), &[]).le(start() + Dt, end()));
        assert!(!timing(quarter, Some(quarter), &[]).lt(start() + Dt, end()));
    }

    #[test]
    fn holds_before_end_of_open_interval() {
        let two = RealValue::from_integer(2);
        let dt = Some(RealValue::from_integer(1));
        let over_all = TimeInterval::open(start(), end());
        // `]start, end[` holds just before `end`
        assert!(timing(two, dt, &[start(), end()]).holds_before(over_all, end()));
        assert!(!timing(two, dt, &[]).holds_before(over_all, end() + Dt));
        // unless an effect of the action may change the value at `end - dt`
        assert!(!timing(two, dt, &[end() - Dt]).holds_before(over_all, end()));
        // outside of an action, the effects are unknown
        let global = ActionTiming {
            effect_starts: None,
            ..timing(two, dt, &[])
        };
        assert!(!global.holds_before(over_all, end()));
    }
}
