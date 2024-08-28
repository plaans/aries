use crate::core::state::{Cause, Domains, Explanation};
use crate::core::{IntCst, Lit, SignedVar, UpperBound, INT_CST_MIN};
use crate::reasoners::cp::{Propagator, PropagatorId, Watches};
use crate::reasoners::Contradiction;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct MaxElem {
    var: SignedVar,
    cst: IntCst,
    presence: Lit,
}

impl MaxElem {
    pub fn new(var: SignedVar, cst: IntCst, presence: Lit) -> Self {
        Self { var, cst, presence }
    }
}

/// Limited propagator that ONLY enforces that the UB of the left element (max) is below the UB of the right elements.
///
/// This is not sufficient to implement a propagator of the Max constraint and is only used as of several propagators in a decomposition.
#[derive(Clone)]
pub(crate) struct LeftUbMax {
    pub max: SignedVar,
    pub elements: Vec<MaxElem>,
}

impl Propagator for LeftUbMax {
    fn setup(&self, id: PropagatorId, context: &mut Watches) {
        for e in &self.elements {
            context.add_watch(e.var.variable(), id);
            context.add_watch(e.presence.variable(), id);
        }
    }

    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        let mut at_least_one_elem = false;
        let mut ub = INT_CST_MIN - 1;
        for e in &self.elements {
            if !domains.entails(!e.presence) {
                at_least_one_elem = true;
                let local_ub = domains.get_bound(e.var).as_int() + e.cst;
                ub = ub.max(local_ub);
            }
        }
        if at_least_one_elem {
            domains.set_ub(self.max, ub, cause)?;
        } else {
            let max_presence = domains.presence(self.max.variable());
            domains.set(!max_presence, cause)?;
        }

        Ok(())
    }

    fn explain(&self, literal: Lit, domains: &Domains, out_explanation: &mut Explanation) {
        let max_ub = if literal.svar() == self.max {
            literal.bound_value().as_int()
        } else {
            domains.get_bound(self.max).as_int()
        };
        // max <= max_ub   <-  And_i  (ei.var + ei.cst) <= max_ub || !ei.prez
        for e in &self.elements {
            if domains.entails(!e.presence) {
                out_explanation.push(!e.presence);
            } else {
                // e.var + e.cst <= max_ub
                // e.var <= max_ub - e.cst
                let lit = Lit::from_parts(e.var, UpperBound::ub(max_ub - e.cst));
                debug_assert!(domains.entails(lit));
                out_explanation.push(lit);
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod test {
    use crate::core::state::{Cause, Domains};
    use crate::core::{IntCst, Lit, SignedVar, VarRef};
    use crate::reasoners::cp::max::{LeftUbMax, MaxElem};
    use crate::reasoners::cp::Propagator;

    fn check_bounds(d: &Domains, v: VarRef, lb: IntCst, ub: IntCst) {
        assert_eq!(d.lb(v), lb);
        assert_eq!(d.ub(v), ub);
    }

    pub static CAUSE: Cause = Cause::Decision;

    #[test]
    /// Tests that the upper bound of a variable can be changed
    fn test_ub_setter_var() {
        let d = &mut Domains::new();

        let m = d.new_var(0, 20);

        let a = d.new_var(0, 10);
        let b = d.new_var(0, 12);

        let c = LeftUbMax {
            max: SignedVar::plus(m),
            elements: vec![
                MaxElem {
                    var: SignedVar::plus(a),
                    cst: 1,
                    presence: Lit::TRUE,
                },
                MaxElem {
                    var: SignedVar::plus(b),
                    cst: 1,
                    presence: Lit::TRUE,
                },
            ],
        };
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 13);

        d.set_ub(a, 9, CAUSE);
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 13);

        d.set_ub(b, 9, CAUSE);
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 10);

        d.set_ub(a, 5, CAUSE);
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 10);

        d.set_ub(b, 3, CAUSE);
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 6);
    }
}
