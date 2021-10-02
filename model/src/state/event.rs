use crate::literals::{BoundValue, Lit, VarBound};
use crate::state::cause::Origin;
use crate::state::int_domains::ValueCause;
use aries_backtrack::EventIndex;

pub type ChangeIndex = Option<EventIndex>;

/// An event represents an update to the domain of a variable.
/// It is typically stored in a trail an provides:
///
/// - the affected variable bound, e.g., lb(x3)
/// - the previous value of the bound. This allows backtracking by undoing the change.
///   The `previous` field also provides the index of the event that set the previous value, to support efficiently
///   scanning the trail.
/// - the new value of the bound. This is available directly in the trail to allow efficiently scanning the trail
///   for the latest changes.
/// - the cause of this event, which can be used for computing explanations.
#[derive(Copy, Clone)]
pub struct Event {
    pub affected_bound: VarBound,
    pub previous: ValueCause,
    pub new_value: BoundValue,
    pub cause: Origin,
}

impl Event {
    #[inline]
    pub fn makes_true(&self, lit: Lit) -> bool {
        debug_assert_eq!(self.affected_bound, lit.affected_bound());
        self.new_value.stronger(lit.bound_value()) && !self.previous.value.stronger(lit.bound_value())
    }

    #[inline]
    pub fn new_literal(&self) -> Lit {
        Lit::from_parts(self.affected_bound, self.new_value)
    }
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} \tprev: {:?} \tcaused_by: {:?}",
            self.affected_bound.bind(self.new_value),
            self.affected_bound.bind(self.previous.value),
            self.cause
        )
    }
}
