use crate::{
    core::{
        state::{OptDomain, RangeDomain},
        views::{Term, VarView},
    },
    prelude::*,
};

/// Trait providing access to the domain of a variable.
///
/// A variable's domain has two components:
///
///  - its *presence*, which is captured by the value of a presence literal.
///    The presence literal associated to a variable can be accessed with the [`presence`](Self::presence) method.
///  - its integral bounds, i.e., the lower and upper bounds on the variable's value *if* it is present.
///    Those can be accessed with the [`lb`](Self::lb) and [`ub`](Self::ub) method respectively.
///
/// This trait provide many useful method accessing the domains and its primarily implemented by the [`Domains`] structure but any type that wraps it
/// can implemented to provide a higher-level interface to access the domains.
pub trait Dom {
    /// Returns the current upper bound of a signed variable, ignoring its presence status.
    ///
    /// This represents one of the fundamental capabilities of a domain but one should in general prefer the more general [`ub`](Self::ub) method.
    fn _upper_bound(&self, svar: SignedVar) -> IntCst;

    /// Returns the presence literal of a variable.
    ///
    /// This represents one of the fundamental capabilities of a domain but one should in general prefer the more general [`presence`](Self::presence) method.
    fn _presence(&self, var: Var) -> Lit;

    /// Returns the current lower bound of a signed variable, ignoring its presence status.
    ///
    /// This represents one of the fundamental capabilities of a domain but one should in general prefer the more general [`lb`](Self::lb) method.
    fn _lower_bound(&self, svar: SignedVar) -> IntCst {
        -self._upper_bound(-svar)
    }

    /// Returns true if the literal is always entailed (or absent).
    fn entails(&self, literal: Lit) -> bool {
        self.ub(literal.svar()) <= literal.ub_value()
    }

    /// Returns the upper bound of the variable, ignoring its presence.
    ///
    /// If the variable is already determined to be absent, will return the upper bound on the integral value it had
    /// before being set to absent.
    fn ub<Var: VarView>(&self, var: Var) -> Var::Value {
        var.upper_bound(self)
    }

    /// Returns the lower bound of the variable, ignoring its presence.
    ///
    /// If the variable is already determined to be absent, will return the lower bound on the integral value it had
    /// before being set to absent.
    fn lb<Var: VarView>(&self, var: Var) -> Var::Value {
        var.lower_bound(self)
    }

    /// Returns a tuple with the lower and upper bound of the variable, ignoring its presence.
    fn bounds<Var: VarView + Copy>(&self, v: Var) -> (Var::Value, Var::Value) {
        let lb = self.lb(v);
        let ub = self.ub(v);
        debug_assert!(
            lb <= ub,
            "this may be the case if the `Ord` of Var::Value is not compatible with the `Ord` of the integer domain"
        );
        (lb, ub)
    }

    /// Returns the domain of the integral part of the variable.
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

    /// Returns the presence literal of a [`Term`] (expression with at most one variable).
    fn presence(&self, term: impl Term) -> Lit {
        self._presence(term.variable())
    }

    fn present(&self, atom: impl Term) -> Option<bool> {
        self.boolean_value_of(self.presence(atom))
    }

    /// Returns the domain of an optional integer expression.
    fn opt_domain_of(&self, atom: impl Into<IAtom>) -> OptDomain {
        let atom = atom.into();
        let (lb, ub) = self.bounds(atom);
        let prez = self.presence(atom.var);
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

impl<X: ?Sized> Dom for &X
where
    X: Dom,
{
    fn _upper_bound(&self, svar: SignedVar) -> IntCst {
        Dom::_upper_bound(*self, svar)
    }

    fn _presence(&self, var: Var) -> Lit {
        Dom::_presence(*self, var)
    }
}
impl<X: ?Sized> Dom for &mut X
where
    X: Dom,
{
    fn _upper_bound(&self, svar: SignedVar) -> IntCst {
        Dom::_upper_bound(*self, svar)
    }

    fn _presence(&self, var: Var) -> Lit {
        Dom::_presence(*self, var)
    }
}

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
