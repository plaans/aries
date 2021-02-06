use crate::int_model::{DomEvent, VarEvent};
use crate::lang::boolean::BVar;
use crate::lang::{IntCst, VarRef};
use core::convert::{From, Into};

/// A `Bound` represents a a lower or upper bound on a discrete variable
/// (i.e. an integer, boolean or symbolic variable).
///
/// For a boolean variable X:
///  - the bound `x > 0` represent the true literal (`X` takes the value `true`)
///  - the bound `x <= 0` represents the false literal (`X` takes the value `false`)
///
/// ```
/// use aries_model::Model;
/// use aries_model::lang::Bound;
/// let mut model = Model::new();
/// let x = model.new_bvar("X");
/// let x_is_true: Bound = x.true_lit();
/// let x_is_false: Bound = x.false_lit();
/// let y = model.new_ivar(0, 10, "Y");
/// let y_geq_5 = y.geq(5);
/// ```
/// TODO: look into bitfields to bring this down to 64 bits
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum Bound {
    LEQ(VarRef, IntCst),
    GT(VarRef, IntCst),
}

impl Bound {
    pub fn variable(&self) -> VarRef {
        match self {
            Bound::LEQ(v, _) => *v,
            Bound::GT(v, _) => *v,
        }
    }

    pub fn leq(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::LEQ(var.into(), val)
    }
    pub fn lt(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::leq(var, val - 1)
    }
    pub fn geq(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::gt(var, val - 1)
    }
    pub fn gt(var: impl Into<VarRef>, val: IntCst) -> Bound {
        Bound::GT(var.into(), val)
    }

    pub fn is_true(v: BVar) -> Bound {
        Bound::geq(v, 1)
    }
    pub fn is_false(v: BVar) -> Bound {
        Bound::leq(v, 0)
    }

    pub fn made_true_by(&self, event: &VarEvent) -> bool {
        let neg = !*self;
        neg.made_false_by(event)
    }
    pub fn made_false_by(&self, event: &VarEvent) -> bool {
        if self.var() != event.var {
            return false;
        }
        match self {
            Bound::LEQ(_, upper_bound) => {
                if let DomEvent::NewLB { prev, new } = event.ev {
                    prev <= *upper_bound && *upper_bound < new
                } else {
                    false
                }
            }
            Bound::GT(_, val) => {
                let lower_bound = val + 1;
                if let DomEvent::NewUB { prev, new } = event.ev {
                    lower_bound > new && prev >= lower_bound
                } else {
                    false
                }
            }
        }
    }

    pub fn entails(&self, other: Bound) -> bool {
        if self.var() != other.var() {
            return false;
        }
        match self {
            Bound::LEQ(_, upper_bound) => {
                if let Bound::LEQ(_, o) = other {
                    o >= *upper_bound
                } else {
                    false
                }
            }
            Bound::GT(_, val) => {
                if let Bound::GT(_, o) = other {
                    o <= *val
                } else {
                    false
                }
            }
        }
    }

    pub fn var(&self) -> VarRef {
        match self {
            Bound::LEQ(v, _) => *v,
            Bound::GT(v, _) => *v,
        }
    }
}

impl std::ops::Not for Bound {
    type Output = Bound;

    fn not(self) -> Self::Output {
        match self {
            Bound::LEQ(var, val) => Bound::GT(var, val),
            Bound::GT(var, val) => Bound::LEQ(var, val),
        }
    }
}

impl From<VarEvent> for Bound {
    fn from(ev: VarEvent) -> Self {
        match ev.ev {
            DomEvent::NewLB { new: new_lb, .. } => Bound::geq(ev.var, new_lb),
            DomEvent::NewUB { new: new_ub, .. } => Bound::leq(ev.var, new_ub),
        }
    }
}

impl std::fmt::Debug for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bound::LEQ(var, val) => write!(f, "{:?} <= {}", var, val),
            Bound::GT(var, val) => write!(f, "{:?} > {}", var, val),
        }
    }
}
