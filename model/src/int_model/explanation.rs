use crate::int_model::{DiscreteModel, DomEvent, InferenceCause, VarEvent};
use crate::lang::{BVar, IntCst, VarRef};

/// TODO: look into bitfields to bring this down to 64 bits
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum ILit {
    LEQ(VarRef, IntCst),
    GT(VarRef, IntCst),
}

impl ILit {
    pub fn variable(&self) -> VarRef {
        match self {
            ILit::LEQ(v, _) => *v,
            ILit::GT(v, _) => *v,
        }
    }

    pub fn leq(var: impl Into<VarRef>, val: IntCst) -> ILit {
        ILit::LEQ(var.into(), val)
    }
    pub fn lt(var: impl Into<VarRef>, val: IntCst) -> ILit {
        ILit::leq(var, val - 1)
    }
    pub fn geq(var: impl Into<VarRef>, val: IntCst) -> ILit {
        ILit::gt(var, val - 1)
    }
    pub fn gt(var: impl Into<VarRef>, val: IntCst) -> ILit {
        ILit::GT(var.into(), val)
    }

    pub fn is_true(v: BVar) -> ILit {
        ILit::geq(v, 1)
    }
    pub fn is_false(v: BVar) -> ILit {
        ILit::leq(v, 0)
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
            ILit::LEQ(_, upper_bound) => {
                if let DomEvent::NewLB { prev, new } = event.ev {
                    prev <= *upper_bound && *upper_bound < new
                } else {
                    false
                }
            }
            ILit::GT(_, val) => {
                let lower_bound = val + 1;
                if let DomEvent::NewUB { prev, new } = event.ev {
                    lower_bound > new && prev >= lower_bound
                } else {
                    false
                }
            }
        }
    }

    pub fn entails(&self, other: ILit) -> bool {
        if self.var() != other.var() {
            return false;
        }
        match self {
            ILit::LEQ(_, upper_bound) => {
                if let ILit::LEQ(_, o) = other {
                    o >= *upper_bound
                } else {
                    false
                }
            }
            ILit::GT(_, val) => {
                if let ILit::GT(_, o) = other {
                    o <= *val
                } else {
                    false
                }
            }
        }
    }

    pub fn var(&self) -> VarRef {
        match self {
            ILit::LEQ(v, _) => *v,
            ILit::GT(v, _) => *v,
        }
    }
}

impl std::ops::Not for ILit {
    type Output = ILit;

    fn not(self) -> Self::Output {
        match self {
            ILit::LEQ(var, val) => ILit::GT(var, val),
            ILit::GT(var, val) => ILit::LEQ(var, val),
        }
    }
}

impl From<VarEvent> for ILit {
    fn from(ev: VarEvent) -> Self {
        match ev.ev {
            DomEvent::NewLB { new: new_lb, .. } => ILit::geq(ev.var, new_lb),
            DomEvent::NewUB { new: new_ub, .. } => ILit::leq(ev.var, new_ub),
        }
    }
}

impl std::fmt::Debug for ILit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ILit::LEQ(var, val) => write!(f, "{:?} <= {}", var, val),
            ILit::GT(var, val) => write!(f, "{:?} > {}", var, val),
        }
    }
}

/// Builder for a conjunction of literals that make the explained literal true
pub struct Explanation {
    pub(crate) lits: Vec<ILit>,
}
impl Explanation {
    pub fn new() -> Self {
        Explanation { lits: Vec::new() }
    }
    pub fn push(&mut self, lit: ILit) {
        self.lits.push(lit)
    }
}
impl Default for Explanation {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Explainer {
    fn explain(&self, cause: InferenceCause, literal: ILit, model: &DiscreteModel, explanation: &mut Explanation);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lit_implication() {
        let a = VarRef::from(0usize);
        let b = VarRef::from(1usize);

        // event for a domain change of [0, X] to [9, X]
        let ea_lb = VarEvent {
            var: a,
            ev: DomEvent::NewLB { prev: 0, new: 9 },
        };
        // event for a domain change of [X, 10] to [X, 1]
        let ea_ub = VarEvent {
            var: a,
            ev: DomEvent::NewUB { prev: 10, new: 1 },
        };

        // ===== lower bounds ======

        let lit = ILit::LEQ(a, 5);
        assert!(lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        let lit = ILit::LEQ(a, 0);
        assert!(lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // was previously violated
        let lit = ILit::LEQ(a, -1);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // ===== upper bounds =====

        let lit = ILit::geq(a, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(lit.made_false_by(&ea_ub));

        let lit = ILit::geq(a, 10);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(lit.made_false_by(&ea_ub));

        // was previously violated
        let lit = ILit::geq(a, 11);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // ===== unrelated variable =====

        // events on b, should not match
        let lit = ILit::LEQ(b, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));
        let lit = ILit::GT(b, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));
    }

    #[test]
    fn test_entailements() {
        let a = VarRef::from(0usize);
        let b = VarRef::from(1usize);
        fn leq(var: VarRef, val: IntCst) -> ILit {
            ILit::leq(var, val)
        };
        fn geq(var: VarRef, val: IntCst) -> ILit {
            ILit::geq(var, val)
        };

        assert!(leq(a, 0).entails(leq(a, 0)));
        assert!(leq(a, 0).entails(leq(a, 1)));
        assert!(!leq(a, 0).entails(leq(a, -1)));

        assert!(!leq(a, 0).entails(leq(b, 0)));
        assert!(!leq(a, 0).entails(leq(b, 1)));
        assert!(!leq(a, 0).entails(leq(b, -1)));

        assert!(geq(a, 0).entails(geq(a, 0)));
        assert!(!geq(a, 0).entails(geq(a, 1)));
        assert!(geq(a, 0).entails(geq(a, -1)));

        assert!(!geq(a, 0).entails(geq(b, 0)));
        assert!(!geq(a, 0).entails(geq(b, 1)));
        assert!(!geq(a, 0).entails(geq(b, -1)));
    }
}
