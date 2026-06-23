use crate::core::literals::Disjunction;
use crate::core::state::{Evaluable, OptDomain};
use crate::core::{IntCst, Lit, SignedVar, Var};
use crate::lang::ValidityScope;
use crate::lang::alternative::NFAlternative;
use crate::lang::max::NFEqMax;
use crate::model::{Label, Model};
use crate::prelude::{Conjunction, DomainsExt, LinSum, Solution};
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
    Eq(Var, Var),
    Neq(Var, Var),
    EqVal(Var, IntCst),
    NeqVal(Var, IntCst),
    Or(Disjunction),
    And(Conjunction),
    LinearLeq(LinSum),
    LinearEq(LinSum),
    LinearNeq(LinSum),
    // TODO: add LinearEq and LinearNeq, and subsume in specialized Variants
    Alternative(NFAlternative),
    EqMax(NFEqMax),
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
            ReifExpr::LinearLeq(l) => write!(f, "{l}"),
            ReifExpr::LinearEq(l) => write!(f, "{l} = 0"),
            ReifExpr::LinearNeq(l) => write!(f, "{l} != 0"),
            ReifExpr::EqMax(em) => write!(f, "{em:?}"),
            ReifExpr::Alternative(alt) => write!(f, "{alt:?}"),
        }
    }
}

impl ReifExpr {
    pub fn scope(&self, presence: impl Fn(Var) -> Lit) -> ValidityScope {
        match self {
            ReifExpr::Lit(l) => ValidityScope::new([presence(l.variable())], []),
            ReifExpr::MaxDiff(diff) => ValidityScope::new([presence(diff.b), presence(diff.a)], []),
            ReifExpr::Eq(a, b) => ValidityScope::new([presence(*a), presence(*b)], []),
            ReifExpr::Neq(a, b) => ValidityScope::new([presence(*a), presence(*b)], []),
            ReifExpr::EqVal(a, _) => ValidityScope::new([presence(*a)], []),
            ReifExpr::NeqVal(a, _) => ValidityScope::new([presence(*a)], []),
            ReifExpr::Or(literals) => ValidityScope::new(
                literals.iter().map(|l| presence(l.variable())),
                literals.iter().filter(|l| presence(l.variable()) == Lit::TRUE),
            ),
            ReifExpr::And(literals) => ValidityScope::new(
                literals.iter().map(|l| presence(l.variable())),
                literals
                    .iter()
                    .map(|l| !l)
                    .filter(|l| presence(l.variable()) == Lit::TRUE),
            ),
            ReifExpr::LinearLeq(lin) | ReifExpr::LinearEq(lin) | ReifExpr::LinearNeq(lin) => {
                ValidityScope::new(lin.variables().map(presence), [])
            }
            ReifExpr::Alternative(alt) => ValidityScope::new([presence(alt.main)], []),
            ReifExpr::EqMax(eq_max) => ValidityScope::new([presence(eq_max.lhs.variable())], []),
        }
    }

    /// Returns true iff a given expression can be negated.
    pub fn negatable(&self) -> bool {
        !matches!(self, ReifExpr::Alternative(_) | ReifExpr::EqMax(_))
    }

    pub fn eval(&self, assignment: &Solution) -> Option<bool> {
        let prez = |var| assignment.present(var).unwrap();
        let value = |var| match assignment.opt_domain_of(var) {
            OptDomain::Present(lb, ub) if lb == ub => lb,
            _ => panic!(),
        };
        let sprez = |svar: SignedVar| prez(svar.variable());
        let svalue = |svar: SignedVar| {
            if svar.is_plus() {
                value(svar.variable())
            } else {
                -value(svar.variable())
            }
        };
        match &self {
            ReifExpr::Lit(l) => {
                if prez(l.variable()) {
                    Some(assignment.value_of(*l).unwrap())
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
                    if prez(l.variable()) && assignment.entails(l) {
                        return Some(true);
                    }
                }
                if lits.iter().all(|l| prez(l.variable()) && assignment.entails(!l)) {
                    return Some(false);
                }
                assert!(lits.iter().any(|l| !prez(l.variable())));
                None
            }
            ReifExpr::And(_) => (!self.clone()).eval(assignment).map(|value| !value),
            ReifExpr::LinearLeq(lin) => lin.evaluate(assignment).map(|value| value <= 0),
            ReifExpr::LinearEq(lin) => lin.evaluate(assignment).map(|value| value == 0),
            ReifExpr::LinearNeq(lin) => lin.evaluate(assignment).map(|value| value != 0),
            ReifExpr::Alternative(NFAlternative { main, alternatives }) => {
                if prez(*main) {
                    let main_value = value(*main);
                    let mut present_alternatives = alternatives.iter().filter(|a| prez(a.var));
                    match present_alternatives.next() {
                        Some(alt) => {
                            // we have at least one alternative
                            if present_alternatives.next().is_some() {
                                // more than on present alternative, constraint is violated
                                Some(false)
                            } else {
                                Some(main_value == value(alt.var) + alt.cst)
                            }
                        }
                        None => {
                            // no alternative, constraint is violated
                            Some(false)
                        }
                    }
                } else {
                    Some(true)
                }
            }
            ReifExpr::EqMax(NFEqMax { lhs, rhs }) => {
                if sprez(*lhs) {
                    let left_value = svalue(*lhs);
                    let right_value = rhs.iter().filter(|e| sprez(e.var)).map(|e| svalue(e.var) + e.cst).max();
                    if let Some(right_value) = right_value {
                        Some(left_value == right_value)
                    } else {
                        Some(false) // no value in the max while the lhs is present
                    }
                } else {
                    Some(true)
                }
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
            ReifExpr::Or(value)
        }
    }
}
impl From<Conjunction> for ReifExpr {
    fn from(value: Conjunction) -> Self {
        // go through a disjunction to reuse the simplications
        // this may be a bit wasteful and coudl beneift from a direct implementation
        // (but conjunctions are pretty rare in most problems)
        !ReifExpr::from(!value)
    }
}

impl Not for ReifExpr {
    type Output = Self;

    fn not(self) -> Self::Output {
        use ReifExpr::*;
        match self {
            Lit(l) => Lit(!l),
            MaxDiff(diff) => MaxDiff(!diff),
            Eq(a, b) => Neq(a, b),
            Neq(a, b) => Eq(a, b),
            EqVal(a, b) => NeqVal(a, b),
            NeqVal(a, b) => EqVal(a, b),
            Or(lits) => And(!lits),
            And(lits) => Or(!lits),
            LinearLeq(lin) => LinearLeq(-lin + 1), // lin > 0 <=> -lin < 0 <=> -lin +1 <= 0
            LinearEq(lin) => LinearNeq(lin),
            LinearNeq(lin) => LinearEq(lin),
            Alternative(_) => panic!("Alternative is a constraint and cannot be negated"),
            EqMax(_) => panic!("EqMax is a constraint and cannot be negated"),
        }
    }
}

/// A difference expression of the form `b - a <= ub` where `a` and `b` are variables.
#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Clone)]
pub struct DifferenceExpression {
    pub b: Var,
    pub a: Var,
    pub ub: IntCst,
}

impl DifferenceExpression {
    pub fn new(b: Var, a: Var, ub: IntCst) -> Self {
        assert_ne!(b, Var::ZERO);
        assert_ne!(a, Var::ZERO);
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

#[cfg(test)]
mod test {
    use crate::{core::Lit, reif::ReifExpr};

    #[test]
    fn test_reif_expr_size() {
        if std::mem::size_of::<Lit>() == 8 {
            assert_eq!(std::mem::size_of::<ReifExpr>(), 40)
        }
    }
}
