use crate::core::literals::Disjunction;
use crate::core::state::{Domains, OptDomain};
use crate::core::{IntCst, Lit, VarRef};
use crate::model::lang::linear::NFLinearLeq;
use crate::model::lang::ValidityScope;
use crate::model::{Label, Model};
use std::fmt::{Debug, Formatter};
use std::ops::Not;

pub trait Reifiable<Lbl> {
    fn decompose(self, model: &mut Model<Lbl>) -> ReifExpr;
}

impl<Lbl: Label, Expr: Into<ReifExpr>> Reifiable<Lbl> for Expr {
    fn decompose(self, _: &mut Model<Lbl>) -> ReifExpr {
        self.into()
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub enum ReifExpr {
    Lit(Lit),
    MaxDiff(DifferenceExpression),
    Eq(VarRef, VarRef),
    Neq(VarRef, VarRef),
    EqVal(VarRef, IntCst),
    NeqVal(VarRef, IntCst),
    Or(Vec<Lit>),
    And(Vec<Lit>),
    Linear(NFLinearLeq),
}

impl std::fmt::Display for ReifExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReifExpr::Lit(l) => write!(f, "{l:?}"),
            ReifExpr::MaxDiff(md) => write!(f, "{md:?}"),
            ReifExpr::Eq(a, b) => write!(f, "({a:?} = {b:?}"),
            ReifExpr::Neq(a, b) => write!(f, "({a:?} != {b:?}"),
            ReifExpr::EqVal(a, b) => write!(f, "({a:?} = {b:?}"),
            ReifExpr::NeqVal(a, b) => write!(f, "({a:?} != {b:?}"),
            ReifExpr::Or(or) => write!(f, "or{or:?}"),
            ReifExpr::And(and) => write!(f, "and{and:?}"),
            ReifExpr::Linear(l) => write!(f, "{l}"),
        }
    }
}

impl ReifExpr {
    pub fn scope(&self, presence: impl Fn(VarRef) -> Lit) -> ValidityScope {
        match self {
            ReifExpr::Lit(l) => ValidityScope::new([presence(l.variable())], []),
            ReifExpr::MaxDiff(diff) => ValidityScope::new([presence(diff.b), presence(diff.a)], []),
            ReifExpr::Eq(a, b) => ValidityScope::new([presence(*a), presence(*b)], []),
            ReifExpr::Neq(a, b) => ValidityScope::new([presence(*a), presence(*b)], []),
            ReifExpr::EqVal(a, _) => ValidityScope::new([presence(*a)], []),
            ReifExpr::NeqVal(a, _) => ValidityScope::new([presence(*a)], []),
            ReifExpr::Or(literals) => ValidityScope::new(
                literals.iter().map(|l| presence(l.variable())),
                literals.iter().copied().filter(|l| presence(l.variable()) == Lit::TRUE),
            ),
            ReifExpr::And(literals) => ValidityScope::new(
                literals.iter().map(|l| presence(l.variable())),
                literals
                    .iter()
                    .map(|&l| !l)
                    .filter(|l| presence(l.variable()) == Lit::TRUE),
            ),
            ReifExpr::Linear(lin) => lin.validity_scope(presence),
        }
    }

    pub fn eval(&self, assignment: &Domains) -> Option<bool> {
        let prez = |var| assignment.present(var).unwrap();
        let value = |var| match assignment.domain(var) {
            OptDomain::Present(lb, ub) if lb == ub => lb,
            _ => panic!(),
        };
        match &self {
            ReifExpr::Lit(l) => {
                if prez(l.variable()) {
                    Some(assignment.value(*l).unwrap())
                } else {
                    None
                }
            }
            ReifExpr::MaxDiff(diff) => {
                if prez(diff.b) && prez(diff.a) {
                    Some(value(diff.b) - value(diff.a) <= diff.ub)
                } else {
                    None
                }
            }
            ReifExpr::Eq(a, b) => {
                if prez(*a) && prez(*b) {
                    Some(value(*a) == value(*b))
                } else {
                    None
                }
            }
            ReifExpr::Neq(a, b) => {
                if prez(*a) && prez(*b) {
                    Some(value(*a) != value(*b))
                } else {
                    None
                }
            }
            ReifExpr::EqVal(a, b) => {
                if prez(*a) {
                    Some(value(*a) == *b)
                } else {
                    None
                }
            }
            ReifExpr::NeqVal(a, b) => {
                if prez(*a) {
                    Some(value(*a) != *b)
                } else {
                    None
                }
            }
            ReifExpr::Or(lits) => {
                for l in lits {
                    if prez(l.variable()) && assignment.entails(*l) {
                        return Some(true);
                    }
                }
                if lits.iter().all(|l| prez(l.variable()) && assignment.entails(!*l)) {
                    return Some(false);
                }
                assert!(lits.iter().any(|l| !prez(l.variable())));
                None
            }
            ReifExpr::And(_) => (!self.clone()).eval(assignment).map(|value| !value),
            ReifExpr::Linear(lin) => {
                let lin = lin.simplify();
                let mut sum = 0;
                for term in &lin.sum {
                    debug_assert!(assignment.entails(term.lit) || assignment.entails(!term.lit));
                    if assignment.entails(term.lit) {
                        assert!(prez(term.var));
                        sum += value(term.var) * term.factor;
                    }
                }
                Some(sum <= lin.upper_bound)
            }
        }
    }
}

impl From<bool> for ReifExpr {
    fn from(value: bool) -> Self {
        ReifExpr::Lit(value.into())
    }
}

impl From<Lit> for ReifExpr {
    fn from(value: Lit) -> Self {
        ReifExpr::Lit(value)
    }
}

impl From<Disjunction> for ReifExpr {
    fn from(value: Disjunction) -> Self {
        if value.is_tautology() {
            ReifExpr::Lit(Lit::TRUE)
        } else if value.literals().is_empty() {
            ReifExpr::Lit(Lit::FALSE)
        } else if value.literals().len() == 1 {
            ReifExpr::Lit(*value.literals().first().unwrap())
        } else {
            ReifExpr::Or(value.into())
        }
    }
}

impl Not for ReifExpr {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            ReifExpr::Lit(l) => ReifExpr::Lit(!l),
            ReifExpr::MaxDiff(diff) => ReifExpr::MaxDiff(!diff),
            ReifExpr::Eq(a, b) => ReifExpr::Neq(a, b),
            ReifExpr::Neq(a, b) => ReifExpr::Eq(a, b),
            ReifExpr::EqVal(a, b) => ReifExpr::NeqVal(a, b),
            ReifExpr::NeqVal(a, b) => ReifExpr::EqVal(a, b),
            ReifExpr::Or(mut lits) => {
                lits.iter_mut().for_each(|l| *l = !*l);
                ReifExpr::And(lits)
            }
            ReifExpr::And(mut lits) => {
                lits.iter_mut().for_each(|l| *l = !*l);
                ReifExpr::Or(lits)
            }
            ReifExpr::Linear(lin) => ReifExpr::Linear(!lin),
        }
    }
}

/// A difference expression of the form `b - a <= ub` where `a` and `b` are variables.
#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Clone)]
pub struct DifferenceExpression {
    pub b: VarRef,
    pub a: VarRef,
    pub ub: IntCst,
}

impl DifferenceExpression {
    pub fn new(b: VarRef, a: VarRef, ub: IntCst) -> Self {
        assert_ne!(b, VarRef::ZERO);
        assert_ne!(a, VarRef::ZERO);
        DifferenceExpression { b, a, ub }
    }
}

impl Debug for DifferenceExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} <= {:?} + {}", self.b, self.a, self.ub)
    }
}

impl Not for DifferenceExpression {
    type Output = Self;

    fn not(self) -> Self::Output {
        DifferenceExpression::new(self.a, self.b, -self.ub - 1)
    }
}
