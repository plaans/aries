use crate::core::state::{FixedDomain, IntDomain, OptDomain, RangeDomain};
use crate::core::views::{Dom, VarView};
use crate::core::*;
use crate::model::lang::linear::LinearSum;
use crate::model::lang::{Atom, Cst, IAtom, Rational, SAtom};
use crate::model::symbols::SymId;
use crate::model::symbols::TypedSym;
use state::Term;

/// Extension methods for an object containing a partial or total assignment to a problem.
pub trait DomainsExt: Dom + Sized {
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
        (self.lb(v), self.ub(v))
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

    /// Returns the fixed-point domain of the linear sum.
    /// Can also be used with a FAtom.
    fn f_domain(&self, sum: impl Into<LinearSum>) -> FixedDomain {
        let sum: LinearSum = sum.into();
        let (lb, ub) = sum
            .terms()
            .iter()
            .fold((sum.constant(), sum.constant()), |(lb, ub), t| {
                let (l, u) = self.bounds(t.var());
                (lb + l * t.factor(), ub + u * t.factor())
            });
        FixedDomain::new(IntDomain::new(lb, ub), sum.denom())
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

    fn sym_value_of(&self, atom: impl Into<SAtom>) -> Option<SymId> {
        self.var_domain(atom.into()).as_singleton()
    }

    fn evaluate(&self, atom: Atom) -> Option<Cst> {
        match atom {
            Atom::Bool(b) => self.value_of(b).map(Cst::Bool),
            Atom::Int(i) => self.var_domain(i).as_singleton().map(Cst::Int),
            Atom::Fixed(f) => self
                .var_domain(f.num)
                .as_singleton()
                .map(|i| Cst::Fixed(Rational::new(i, f.denom))),
            Atom::Sym(s) => self.sym_value_of(s).map(|sym| Cst::Sym(TypedSym::new(sym, s.tpe()))),
        }
    }

    /// Returns the value of a boolean atom if it as a set value.
    /// Return None otherwise meaning the value con be
    ///  - either true or false
    ///  - neither true nor false (empty domain)
    fn boolean_value_of(&self, bool_atom: impl Into<Lit>) -> Option<bool> {
        self.value_of(bool_atom.into())
    }

    /// Return an integer view of the domain of any kind of atom.
    fn int_bounds(&self, atom: impl Into<Atom>) -> (IntCst, IntCst) {
        let atom = atom.into();
        match atom {
            Atom::Bool(atom) => match self.boolean_value_of(atom) {
                Some(true) => (1, 1),
                Some(false) => (0, 0),
                None => (0, 1),
            },
            Atom::Int(atom) => self.bounds(atom),
            Atom::Fixed(atom) => self.bounds(atom.num),
            Atom::Sym(atom) => self.bounds(atom.int_view()),
        }
    }
}

impl<D> DomainsExt for D where D: Dom + Sized {}

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
