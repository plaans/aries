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
    Or(Vec<Lit>),
    And(Vec<Lit>),
    Linear(NFLinearLeq),
}

impl ReifExpr {
    pub fn scope(&self, presence: impl Fn(VarRef) -> Lit) -> ValidityScope {
        match self {
            ReifExpr::Lit(l) => ValidityScope::new([presence(l.variable())], []),
            ReifExpr::MaxDiff(diff) => ValidityScope::new([presence(diff.b), presence(diff.a)], []),
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
                let mut sum = 0;
                for term in &lin.sum {
                    if assignment.entails(term.lit) {
                        assert!(prez(term.var));
                        sum += value(term.var) * term.factor
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
