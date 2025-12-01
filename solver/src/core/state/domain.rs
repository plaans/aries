use crate::{
    core::{cst_int_to_long, IntCst, LongCst, INT_CST_MAX, INT_CST_MIN},
    model::lang::Rational,
};
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
    pub fn size(&self) -> LongCst {
        cst_int_to_long(self.ub) + cst_int_to_long(self.lb) + 1
    }

    /// Returns true if the domain contains exactly one value.
    pub fn is_bound(&self) -> bool {
        self.lb == self.ub
    }

    /// Returns true if the domain *only* contains `value`
    pub fn is_bound_to(&self, value: IntCst) -> bool {
        self.lb == value && self.ub == value
    }

    /// Returns true if the domain contains `value`
    pub fn contains(&self, value: IntCst) -> bool {
        self.lb <= value && value <= self.ub
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

    /// Returns true if the two domains have no common value
    pub fn disjoint(&self, other: &IntDomain) -> bool {
        self.ub < other.lb || other.ub < self.lb
    }

    /// Returns true if two domains have a non-empty intersection.
    pub fn overlaps(&self, other: &IntDomain) -> bool {
        !self.disjoint(other)
    }
}

impl std::ops::Mul for IntDomain {
    type Output = IntDomain;

    fn mul(self, rhs: Self) -> Self::Output {
        fn max(xs: &[IntCst; 4]) -> IntCst {
            xs[0].max(xs[1]).max(xs[2]).max(xs[3])
        }
        fn min(xs: &[IntCst; 4]) -> IntCst {
            xs[0].min(xs[1]).min(xs[2]).min(xs[3])
        }

        // compute bounds of f1 * f2
        let potential_extrema = [
            self.lb.saturating_mul(rhs.lb),
            self.lb.saturating_mul(rhs.ub),
            self.ub.saturating_mul(rhs.lb),
            self.ub.saturating_mul(rhs.ub),
        ];
        let ub = max(&potential_extrema).clamp(INT_CST_MIN, INT_CST_MAX);
        let lb = min(&potential_extrema).clamp(INT_CST_MIN, INT_CST_MAX);
        IntDomain::new(lb, ub)
    }
}

impl std::fmt::Display for IntDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_bound() {
            write!(f, "{}", self.lb)
        } else if self.is_empty() {
            write!(f, "∅")
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

    pub fn lb(&self) -> Rational {
        Rational::new(self.num.lb, self.denom)
    }
    pub fn ub(&self) -> Rational {
        Rational::new(self.num.lb, self.denom)
    }

    pub fn lb_f32(&self) -> f32 {
        (self.num.lb as f32) / (self.denom as f32)
    }

    pub fn ub_f32(&self) -> f32 {
        (self.num.ub as f32) / (self.denom as f32)
    }
}

impl Display for FixedDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_bound() {
            write!(f, "{:.3}", self.lb_f32())
        } else if self.is_empty() {
            write!(f, "∅")
        } else {
            write!(f, "[{:.3}, {:.3}]", self.lb_f32(), self.ub_f32())
        }
    }
}
