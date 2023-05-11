use crate::core::IntCst;
use std::fmt::{Display, Formatter};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct IntDomain {
    pub lb: IntCst,
    pub ub: IntCst,
}
impl IntDomain {
    pub fn new(lb: IntCst, ub: IntCst) -> IntDomain {
        IntDomain { lb, ub }
    }

    /// Returns the number of elements in the domain.
    pub fn size(&self) -> i64 {
        (self.ub as i64) - (self.lb as i64) + 1
    }

    /// Returns true if the domain contains exactly one value.
    pub fn is_bound(&self) -> bool {
        self.lb == self.ub
    }

    /// If the domain contains a single value, return it.
    /// Returns `None` otherwise.
    pub fn as_singleton(&self) -> Option<IntCst> {
        if self.is_bound() {
            Some(self.lb)
        } else {
            None
        }
    }

    /// Returns true if the domain is empty.
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

/// Represents the domain of an optional variable
#[derive(Eq, PartialEq, Copy, Clone)]
pub enum OptDomain {
    /// The variable is necessarily present and must take a value in the given literals.
    Present(IntCst, IntCst),
    /// It is unknown whether the variable is present but if it is it must take a value in the given literals.
    Unknown(IntCst, IntCst),
    /// The variable is known to be absent
    Absent,
}

impl std::fmt::Debug for OptDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            OptDomain::Present(lb, ub) if lb == ub => write!(f, "[{lb}]"),
            OptDomain::Present(lb, ub) => write!(f, "[{lb}, {ub}]"),
            OptDomain::Unknown(lb, ub) if lb == ub => write!(f, "?[{lb}]"),
            OptDomain::Unknown(lb, ub) => write!(f, "?[{lb}, {ub}]"),
            OptDomain::Absent => write!(f, "_"),
        }
    }
}

/// Domain of a fixed-point expression.
pub struct FixedDomain {
    pub num: IntDomain,
    pub denom: IntCst,
}

impl FixedDomain {
    pub fn new(num: IntDomain, denom: IntCst) -> FixedDomain {
        FixedDomain { num, denom }
    }

    pub fn is_bound(&self) -> bool {
        self.num.is_bound()
    }

    pub fn is_empty(&self) -> bool {
        self.num.is_empty()
    }

    pub fn lb(&self) -> f32 {
        (self.num.lb as f32) / (self.denom as f32)
    }

    pub fn ub(&self) -> f32 {
        (self.num.ub as f32) / (self.denom as f32)
    }
}

impl Display for FixedDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_bound() {
            write!(f, "{:.3}", self.lb())
        } else if self.is_empty() {
            write!(f, "none")
        } else {
            write!(f, "[{:.3}, {:.3}]", self.lb(), self.ub())
        }
    }
}
