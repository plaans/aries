use crate::core::state::{Cause, Domains, DomainsSnapshot, Explanation};
use crate::core::{INT_CST_MIN, IntCst, Lit, SignedVar};
use crate::reasoners::Contradiction;
use crate::reasoners::cp::{Propagator, PropagatorId, Watches};

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

/// Propagator for a constraint that enforces that at least one element from the RHS is present and greater than or
/// equal to the element at the LHS. The scope of the propagator is the presence of the LHS.
///
/// Constraint:  `prez(lhs)   =>    OR_i  prez(rhs[i]) & (rhs[i] >= lhs)`
/// Assumes that:   `forall i , prez(rhs[i]) => prez(lhs)`  i.e.  RHS elements are in the (sub?)scope of LHS
///
/// This is not sufficient to implement a propagator of the Max constraint and is only used as one of several propagators in a decomposition.
#[derive(Clone)]
pub(crate) struct AtLeastOneGeq {
    /// presence of LHS and scope of the constraint
    pub scope: Lit,
    pub lhs: SignedVar,
    pub elements: Vec<MaxElem>,
}

impl Propagator for AtLeastOneGeq {
    fn setup(&self, id: PropagatorId, context: &mut Watches) {
        context.add_watch(self.scope.variable(), id);
        context.add_watch(self.lhs.variable(), id);
        for e in &self.elements {
            context.add_watch(e.var.variable(), id);
            context.add_watch(e.presence.variable(), id);
        }
    }

    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        if domains.entails(!self.scope) {
            return Ok(()); // inactive, skip propagation
        }
        enum Candidates {
            Empty,
            Single(usize),
            Several,
        }
        let ub = |svar: SignedVar| domains.ub(svar);
        let lb = |svar: SignedVar| domains.lb(svar);

        let mut candidates = Candidates::Empty;
        let lhs_ub = ub(self.lhs);
        let lhs_lb = lb(self.lhs);
        let mut rhs_max = INT_CST_MIN - 1;

        for (idx, elem) in self.elements.iter().enumerate() {
            if domains.entails(!elem.presence) {
                continue; // elem is provably absent
            }
            let elem_ub = ub(elem.var) + elem.cst;
            if elem_ub < lhs_lb {
                continue; // elem cannot reach the value of the lhs
            }
            // we have one more candidate that may be present and be geq to the lhs
            rhs_max = rhs_max.max(elem_ub);
            candidates = match candidates {
                Candidates::Empty => Candidates::Single(idx),
                _ => Candidates::Several,
            }
        }

        match candidates {
            Candidates::Empty => {
                let max_presence = domains.presence(self.lhs.variable());
                domains.set(!max_presence, cause)?; // PROP 1
            }
            _ => {
                domains.set_ub(self.lhs, rhs_max, cause)?; // PROP 2
            }
        }
        if let Candidates::Single(idx) = candidates {
            let elem = &self.elements[idx];
            // lb(elem.var + elem.cst) <- lb(lhs)
            // lb(elem.var) <- lb(lhs) - elem.cst
            domains.set_lb(elem.var, domains.lb(self.lhs) - elem.cst, cause)?; // PROP 3
            if domains.entails(domains.presence(self.lhs)) {
                // if the constraint is active, the elem must be present
                domains.set(elem.presence, cause)?; // PROP 4
            }
        }

        Ok(())
    }

    fn explain(&self, literal: Lit, domains: &DomainsSnapshot, out_explanation: &mut Explanation) {
        debug_assert_eq!(self.scope, domains.presence(self.lhs.variable()));
        if literal == !self.scope {
            // PROP 1
            let max_lb = domains.lb(self.lhs);
            // !max_prez  <-  And_i  !ei.prez || (ei.var + ei.cst) < max_lb

            for e in &self.elements {
                if domains.entails(!e.presence) {
                    out_explanation.push(!e.presence);
                } else {
                    // e.var + e.cst < max_lb
                    // e.var < max_lb - e.cst
                    let lit = Lit::lt(e.var, max_lb - e.cst);
                    debug_assert!(domains.entails(lit));
                    out_explanation.push(lit);
                    out_explanation.push(Lit::geq(self.lhs, max_lb))
                }
            }
        } else if literal.svar() == self.lhs {
            // PROP 2
            // max <= max_ub   <-  And_i  (ei.var + ei.cst) <= max_ub || !ei.prez
            let max_ub = literal.ub_value();

            for e in &self.elements {
                if domains.entails(!e.presence) {
                    out_explanation.push(!e.presence);
                } else {
                    // e.var + e.cst <= max_ub
                    // e.var <= max_ub - e.cst
                    let lit = e.var.leq(max_ub - e.cst);
                    debug_assert!(domains.entails(lit));
                    out_explanation.push(lit);
                }
            }
        } else {
            // PROP 3 or 4, find the element that was propagated
            let (idx, elem) = self
                .elements
                .iter()
                .enumerate()
                .find(|(i, e)| literal == e.presence || literal.svar() == -e.var)
                .unwrap();

            let max_lb = domains.lb(self.lhs);
            // !max_prez  <-  And_i  !ei.prez || (ei.var + ei.cst) < max_lb

            // propagation made possible because all other elements were inapplicable
            for (_, e) in self.elements.iter().enumerate().filter(|(i, _)| *i != idx) {
                if domains.entails(!e.presence) {
                    out_explanation.push(!e.presence);
                } else {
                    // e.var + e.cst < max_lb
                    // e.var < max_lb - e.cst
                    let lit = Lit::lt(e.var, max_lb - e.cst);
                    debug_assert!(domains.entails(lit));
                    out_explanation.push(lit);
                    out_explanation.push(Lit::geq(self.lhs, max_lb))
                }
            }
            if literal == elem.presence {
                // PROP 4
                out_explanation.push(self.scope) // TODO: should this always be there as the scope ?
            } else {
                // PROP 3
                let inferrable = Lit::geq(elem.var, max_lb - elem.cst);
                debug_assert!(inferrable.entails(literal));
                out_explanation.push(Lit::geq(self.lhs, max_lb));
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
    use crate::reasoners::cp::Propagator;
    use crate::reasoners::cp::max::{AtLeastOneGeq, MaxElem};

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

        let c = AtLeastOneGeq {
            scope: Lit::TRUE,
            lhs: SignedVar::plus(m),
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
