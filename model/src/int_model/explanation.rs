use crate::int_model::{DiscreteModel, DomEvent, InferenceCause, VarEvent};
use crate::lang::{BVar, Bound, IntCst, VarRef};

/// Builder for a conjunction of literals that make the explained literal true
pub struct Explanation {
    pub(crate) lits: Vec<Bound>,
}
impl Explanation {
    pub fn new() -> Self {
        Explanation { lits: Vec::new() }
    }
    pub fn push(&mut self, lit: Bound) {
        self.lits.push(lit)
    }
}
impl Default for Explanation {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Explainer {
    fn explain(&self, cause: InferenceCause, literal: Bound, model: &DiscreteModel, explanation: &mut Explanation);
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

        let lit = Bound::LEQ(a, 5);
        assert!(lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        let lit = Bound::LEQ(a, 0);
        assert!(lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // was previously violated
        let lit = Bound::LEQ(a, -1);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // ===== upper bounds =====

        let lit = Bound::geq(a, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(lit.made_false_by(&ea_ub));

        let lit = Bound::geq(a, 10);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(lit.made_false_by(&ea_ub));

        // was previously violated
        let lit = Bound::geq(a, 11);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));

        // ===== unrelated variable =====

        // events on b, should not match
        let lit = Bound::LEQ(b, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));
        let lit = Bound::GT(b, 5);
        assert!(!lit.made_false_by(&ea_lb));
        assert!(!lit.made_false_by(&ea_ub));
    }

    #[test]
    fn test_entailements() {
        let a = VarRef::from(0usize);
        let b = VarRef::from(1usize);
        fn leq(var: VarRef, val: IntCst) -> Bound {
            Bound::leq(var, val)
        };
        fn geq(var: VarRef, val: IntCst) -> Bound {
            Bound::geq(var, val)
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
