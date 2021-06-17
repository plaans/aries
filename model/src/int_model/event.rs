use crate::bounds::{Bound, BoundValue, VarBound};
use crate::int_model::int_domains::ValueCause;
use crate::int_model::Cause;
use aries_backtrack::EventIndex;

pub type ChangeIndex = Option<EventIndex>;

#[derive(Copy, Clone)]
pub struct Event {
    pub affected_bound: VarBound,
    pub previous: ValueCause,
    pub new_value: BoundValue,
    pub cause: Cause,
}

impl Event {
    #[inline]
    pub fn makes_true(&self, lit: Bound) -> bool {
        debug_assert_eq!(self.affected_bound, lit.affected_bound());
        self.new_value.stronger(lit.bound_value()) && !self.previous.value.stronger(lit.bound_value())
    }

    #[inline]
    pub fn new_literal(&self) -> Bound {
        Bound::from_parts(self.affected_bound, self.new_value)
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
