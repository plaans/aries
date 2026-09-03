use crate::core::state::{Cause, Domains, DomainsSnapshot, Explanation};
use crate::core::views::{IntBoundable, Term, VarView};
use crate::core::{INT_CST_MIN, IntCst, Lit};
use crate::prelude::*;
use crate::reasoners::Contradiction;
use crate::reasoners::cp::{DynPropagator, Propagator, PropagatorId, UserPropagator, Watches};
use std::fmt::Debug;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct MaxElem<Variable> {
    /// Variable providing a potential maximum value (ignored when absent).
    var: Variable,
    /// Presence literal of the variable.
    /// This is redundant and placed here to avoid the need to retrieve from the domains on every access.
    presence: Lit,
}

impl<Variable> MaxElem<Variable> {
    pub fn new(variable: Variable, presence: Lit) -> Self {
        Self {
            var: variable,
            presence,
        }
    }
}

/// Propagator for a constraint that enforces that at least one element from the RHS is present and greater than or
/// equal to the element at the LHS. The scope of the propagator is the presence of the LHS.
///
/// Constraint:  `prez(lhs)   =>    OR_i  prez(rhs[i]) & (rhs[i] >= lhs)`
/// Assumes that:   `forall i , prez(rhs[i]) => prez(lhs)`  i.e.  RHS elements are in the (sub?)scope of LHS
///
/// This is not sufficient to implement a propagator of the Max constraint and is only used as one of several propagators in a decomposition.
#[derive(Clone, Debug)]
pub(crate) struct AtLeastOneGeq<Variable>
where
    Variable: Send + Sync,
{
    /// scope of the constraint.
    pub scope: Lit,
    /// True if the constraint must be active.
    /// It must be the case that `scope => prez(active)`
    pub active: Lit,
    /// It must be the case that `scope => prez(lhs)`
    pub lhs: Variable,
    pub elements: Vec<MaxElem<Variable>>,
}

impl<Variable> Propagator for AtLeastOneGeq<Variable>
where
    Variable: Term + VarView<Value = IntCst> + IntBoundable + Send + Sync + Copy + 'static,
{
    fn setup(&self, id: PropagatorId, context: &mut Watches) {
        context.add_watch(self.scope, id);
        context.add_watch(self.active, id);
        context.add_watch(self.lhs, id);
        for e in &self.elements {
            context.add_watch(e.var, id);
            context.add_watch(e.presence, id);
        }
    }

    fn propagate(&mut self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        if domains.entails(!self.scope) || domains.entails(!self.active) {
            return Ok(()); // inactive or not in scope, skip propagation
        }
        enum Candidates {
            Empty,
            Single(usize),
            Several,
        }

        let mut candidates = Candidates::Empty;
        let lhs_lb = domains.lb(self.lhs);
        let mut rhs_max = INT_CST_MIN - 1;

        for (idx, elem) in self.elements.iter().enumerate() {
            if domains.entails(!elem.presence) {
                continue; // elem is provably absent
            }
            let elem_ub = domains.ub(elem.var);
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

        if matches!(candidates, Candidates::Empty) {
            // no element can be GEQ than lhs.
            //
            // There are three possibilities where this is valid:
            //  - !active: the constraint is inactive
            //  - !scope: the constraint is not in scope
            //  - !prez(lhs): the lhs is absent
            //
            // So be can set: !active \/ !scope \/ !prez(lhs)
            //
            // note that `!prez(lhs) => !scope`, so we can simplify this to
            //   !active \/ !scope
            //
            // Since scope <=> prez(active), this is equivalent to setting !active
            domains.set(!self.active, cause)?; // PROP 1
            return Ok(());
        }

        if domains.entails(self.active) {
            // the constraint is active, we can thus propagate to the variables

            // lhs cannot be greater that the biggest element of rhs
            domains.set_ub(self.lhs, rhs_max, cause)?; // PROP 2

            if let Candidates::Single(idx) = candidates {
                // we have a single element can be GEQ than lhs
                let elem = &self.elements[idx];
                // Tighten lower bound of elem, which is only possible if `prez(var) => prez(lhs)` (which is an assumption here)
                // lb(elem) <- lb(lhs)
                domains.set_lb(elem.var, domains.lb(self.lhs), cause)?; // PROP 3
                if domains.entails(domains.presence(self.lhs)) {
                    // if lhs is present, the elem must be present
                    domains.set(elem.presence, cause)?; // PROP 4
                }
            }
        }

        Ok(())
    }

    fn explain(&self, literal: Lit, domains: &DomainsSnapshot, out_explanation: &mut Explanation) {
        debug_assert_eq!(self.scope, domains.presence(self.lhs.variable()));
        if literal == !self.active {
            // PROP 1
            let max_lb = domains.lb(self.lhs);
            // !max_prez  <-  And_i  !ei.prez || (ei < max_lb)

            for e in &self.elements {
                if domains.entails(!e.presence) {
                    out_explanation.push(!e.presence);
                } else {
                    // e.var + e.cst < max_lb
                    // e.var < max_lb - e.cst
                    let lit = e.var.leq(max_lb - 1);
                    debug_assert!(domains.entails(lit));
                    out_explanation.push(lit);
                    out_explanation.push(self.lhs.geq(max_lb));
                }
            }
        } else if literal.svar() == self.lhs.upper_bounding_signed_var() {
            // PROP 2
            // max <= max_ub   <-  And_i  (ei.var + ei.cst) <= max_ub || !ei.prez
            let max_ub = literal.ub_value();

            out_explanation.push(self.active);

            for e in &self.elements {
                if domains.entails(!e.presence) {
                    out_explanation.push(!e.presence);
                } else {
                    let lit = e.var.leq(max_ub);
                    debug_assert!(domains.entails(lit));
                    out_explanation.push(lit);
                }
            }
        } else {
            // all these propagations required the constraint to be active
            out_explanation.push(self.active);

            // PROP 3 or 4, find the element that was propagated
            let (idx, elem) = self
                .elements
                .iter()
                .enumerate()
                .find(|(_, e)| literal == e.presence || literal.svar() == e.var.lower_bounding_signed_var())
                .unwrap();

            let max_lb = domains.lb(self.lhs);
            // !max_prez  <-  And_i  !ei.prez || (ei.var + ei.cst) < max_lb

            // propagation made possible because all other elements were inapplicable
            for (_, e) in self.elements.iter().enumerate().filter(|(i, _)| *i != idx) {
                if domains.entails(!e.presence) {
                    out_explanation.push(!e.presence);
                } else {
                    let lit = e.var.lt(max_lb);
                    debug_assert!(domains.entails(lit));
                    out_explanation.push(lit);
                    out_explanation.push(self.lhs.geq(max_lb))
                }
            }
            if literal == elem.presence {
                // PROP 4
                out_explanation.push(self.scope) // TODO: should this always be there as the scope ?
            } else {
                // PROP 3
                let inferrable = elem.var.geq(max_lb);
                debug_assert!(inferrable.entails(literal));
                out_explanation.push(self.lhs.geq(max_lb));
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

impl<Variable> UserPropagator for AtLeastOneGeq<Variable>
where
    Variable: Term + VarView<Value = IntCst> + IntBoundable + Send + Sync + Copy + Debug + 'static,
{
    fn get_propagators(&self) -> Vec<super::DynPropagator> {
        vec![DynPropagator::from(self.clone())]
    }

    fn satisfied(&self, sol: &Solution) -> bool {
        match sol.eval(self.lhs) {
            Some(val) => self
                .elements
                .iter()
                .any(|rhs_term| sol.eval(rhs_term.var).is_some_and(|rhs_val| rhs_val >= val)),
            None => true,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::core::state::{Cause, Domains};
    use crate::core::{IntCst, Lit, SignedVar, Var};
    use crate::reasoners::cp::Propagator;
    use crate::reasoners::cp::max::{AtLeastOneGeq, MaxElem};

    fn check_bounds(d: &Domains, v: Var, lb: IntCst, ub: IntCst) {
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

        let mut c = AtLeastOneGeq {
            scope: Lit::TRUE,
            active: Lit::TRUE,
            lhs: SignedVar::plus(m) + 0,
            elements: vec![
                MaxElem {
                    var: SignedVar::plus(a) + 1,
                    presence: Lit::TRUE,
                },
                MaxElem {
                    var: SignedVar::plus(b) + 1,
                    presence: Lit::TRUE,
                },
            ],
        };
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 13);

        d.set_ub(a, 9, CAUSE).unwrap();
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 13);

        d.set_ub(b, 9, CAUSE).unwrap();
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 10);

        d.set_ub(a, 5, CAUSE).unwrap();
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 10);

        d.set_ub(b, 3, CAUSE).unwrap();
        c.propagate(d, CAUSE).unwrap();
        check_bounds(d, m, 0, 6);
    }
}
