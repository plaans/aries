use crate::core::state::{OptDomain, RangeDomain};
use crate::core::views::{Dom, Term, VarView};
use crate::core::*;
use crate::model::lang::IAtom;

/// Extension methods for an object containing a partial or total assignment to a problem.
pub trait DomainsExt: Dom {
    fn entails(&self, literal: Lit) -> bool {
        Dom::upper_bound(self, literal.svar()) <= literal.ub_value()
    }

    /// Returns the upper bound of the variable.
    fn ub<Var: VarView>(&self, var: Var) -> Var::Value {
        var.upper_bound(self)
    }

    /// Returns the lower bound of the variable.
    fn lb<Var: VarView>(&self, var: Var) -> Var::Value {
        var.lower_bound(self)
    }
    fn bounds<Var: VarView + Copy>(&self, v: Var) -> (Var::Value, Var::Value) {
        let lb = self.lb(v);
        let ub = self.ub(v);
        debug_assert!(
            lb <= ub,
            "this may be the case if the `Ord` of Var::Value is not compatible with the `Ord` of the integer domain"
        );
        (lb, ub)
    }

    fn var_domain<Var: VarView + Copy>(&self, var: Var) -> RangeDomain<Var::Value> {
        let (lb, ub) = self.bounds(var);
        RangeDomain::new(lb, ub)
    }

    fn value_of<Var: VarView + Copy>(&self, var: Var) -> Option<Var::Value>
    where
        Var::Value: Copy,
    {
        self.var_domain(var).as_singleton()
    }

    fn presence_literal(&self, variable: impl Term) -> Lit {
        Dom::presence(self, variable.variable())
    }

    fn present(&self, atom: impl Term) -> Option<bool> {
        self.boolean_value_of(self.presence_literal(atom))
    }

    /// Returns the domain of an optional integer expression.
    fn opt_domain_of(&self, atom: impl Into<IAtom>) -> OptDomain {
        let atom = atom.into();
        let (lb, ub) = self.bounds(atom);
        let prez = self.presence_literal(atom.var);
        match self.value_of(prez) {
            Some(true) => OptDomain::Present(lb, ub),
            Some(false) => OptDomain::Absent,
            None => OptDomain::Unknown(lb, ub),
        }
    }

    /// Returns the value of a boolean atom if it as a set value.
    /// Return None otherwise meaning the value con be
    ///  - either true or false
    ///  - neither true nor false (empty domain)
    fn boolean_value_of(&self, bool_atom: impl Into<Lit>) -> Option<bool> {
        self.value_of(bool_atom.into())
    }
}

impl<D: ?Sized> DomainsExt for D where D: Dom {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::state::Domains;

    #[test]
    fn test_lit_domain() {
        let mut domains = Domains::new();

        assert_eq!(domains.var_domain(Lit::TRUE), RangeDomain::new(true, true));
        assert_eq!(domains.var_domain(Lit::FALSE), RangeDomain::new(false, false));

        let var = domains.new_var(0, 1);
        assert_eq!(domains.var_domain(var.leq(0)), RangeDomain::new(false, true));
        assert_eq!(domains.var_domain(var.geq(1)), RangeDomain::new(false, true));
        assert_eq!(domains.var_domain(var.leq(1)), RangeDomain::new(true, true));
        assert_eq!(domains.var_domain(var.geq(2)), RangeDomain::new(false, false));
    }
}
