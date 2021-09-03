use crate::lang::IntCst;

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct IntDomain {
    pub lb: IntCst,
    pub ub: IntCst,
}
impl IntDomain {
    pub fn new(lb: IntCst, ub: IntCst) -> IntDomain {
        IntDomain { lb, ub }
    }

    pub fn size(&self) -> i64 {
        (self.ub as i64) - (self.lb as i64)
    }

    pub fn is_bound(&self) -> bool {
        self.lb == self.ub
    }

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
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum OptDomain {
    /// The variable is necessarily present and must take a value in the given bounds.
    Present(IntCst, IntCst),
    /// It is unknown whether the variable is present but if it is it must take a value in the given bounds.
    Unknown(IntCst, IntCst),
    /// The variable is known to be absent
    Absent,
}
