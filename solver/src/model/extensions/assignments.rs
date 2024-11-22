use crate::core::state::{FixedDomain, IntDomain, OptDomain};
use crate::core::*;
use crate::model::extensions::SavedAssignment;
use crate::model::lang::linear::LinearSum;
use crate::model::lang::{Atom, Cst, IAtom, IVar, Rational, SAtom};
use crate::model::symbols::SymId;
use crate::model::symbols::{ContiguousSymbols, TypedSym};
/// Extension methods for an object containing a partial or total assignment to a problem.
pub trait AssignmentExt {
    fn entails(&self, literal: Lit) -> bool;

    fn var_domain(&self, var: impl Into<IAtom>) -> IntDomain;

    fn presence_literal(&self, variable: VarRef) -> Lit;

    fn value_of_literal(&self, literal: Lit) -> Option<bool> {
        if self.entails(literal) {
            Some(true)
        } else if self.entails(!literal) {
            Some(false)
        } else {
            None
        }
    }
    fn is_undefined_literal(&self, literal: Lit) -> bool {
        self.value_of_literal(literal).is_none()
    }

    fn present(&self, atom: impl Into<Atom>) -> Option<bool> {
        let atom = atom.into();
        let var = atom.variable();
        self.boolean_value_of(self.presence_literal(var))
    }

    fn sym_present(&self, atom: impl Into<SAtom>) -> Option<bool> {
        let atom = atom.into();
        match atom {
            SAtom::Var(v) => self.boolean_value_of(self.presence_literal(v.into())),
            SAtom::Cst(_) => Some(true),
        }
    }

    /// Returns the fixed-point domain of the linear sum.
    /// Can also be used with a FAtom.
    fn f_domain(&self, sum: impl Into<LinearSum>) -> FixedDomain {
        let sum: LinearSum = sum.into();
        let (lb, ub) = sum
            .terms()
            .iter()
            .fold((sum.constant(), sum.constant()), |(lb, ub), t| {
                let (l, u) = self.domain_of(t.var());
                (lb + l * t.factor(), ub + u * t.factor())
            });
        FixedDomain::new(IntDomain::new(lb, ub), sum.denom())
    }

    fn domain_of(&self, atom: impl Into<IAtom>) -> (IntCst, IntCst) {
        let atom = atom.into();
        let base = self.var_domain(atom.var);
        (base.lb + atom.shift, base.ub + atom.shift)
    }

    /// Returns the domain of an optional integer expression.
    fn opt_domain_of(&self, atom: impl Into<IAtom>) -> OptDomain {
        let atom = atom.into();
        let (lb, ub) = self.domain_of(atom);
        let prez = self.presence_literal(atom.var.into());
        match self.value_of_literal(prez) {
            Some(true) => OptDomain::Present(lb, ub),
            Some(false) => OptDomain::Absent,
            None => OptDomain::Unknown(lb, ub),
        }
    }

    fn to_owned_assignment(&self) -> SavedAssignment;

    fn lower_bound(&self, int_var: IVar) -> IntCst {
        self.var_domain(int_var).lb
    }

    fn upper_bound(&self, int_var: IVar) -> IntCst {
        self.var_domain(int_var).ub
    }

    fn sym_domain_of(&self, atom: impl Into<SAtom>) -> ContiguousSymbols {
        let atom = atom.into();
        let (lb, ub) = self.int_bounds(atom);
        let lb = lb as usize;
        let ub = ub as usize;
        ContiguousSymbols::new(SymId::from(lb), SymId::from(ub))
    }

    fn sym_value_of(&self, atom: impl Into<SAtom>) -> Option<SymId> {
        self.sym_domain_of(atom).into_singleton()
    }

    fn evaluate(&self, atom: Atom) -> Option<Cst> {
        match atom {
            Atom::Bool(b) => self.value_of_literal(b).map(Cst::Bool),
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
        self.value_of_literal(bool_atom.into())
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
            Atom::Int(atom) => self.domain_of(atom),
            Atom::Fixed(atom) => self.domain_of(atom.num),
            Atom::Sym(atom) => self.domain_of(atom.int_view()),
        }
    }
}
