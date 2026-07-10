use crate::core::literals::Disjunction;
use crate::core::state::Evaluable;
use crate::core::{Lit, Var};
use crate::lang::ValidityScope;
use crate::model::{Label, Model};
use crate::prelude::{Conjunction, Dom, LinSum, Solution};
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
    /// Requires that at least one of a set of literal is true.
    Or(Disjunction),
    /// Requires that all literals of a set are true.
    And(Conjunction),
    /// Requires a linear sum to be lesser than or equal to 0 (`sum <= 0`).
    LinearLeq(LinSum),
    /// Requires a linear sum to be equal to 0 (`sum == 0`).
    LinearEq(LinSum),
    /// Requires a linear sum to *not* be equal to 0 (`sum != 0`).
    LinearNeq(LinSum),
}

impl std::fmt::Display for ReifExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReifExpr::Lit(l) => write!(f, "{l:?}"),
            ReifExpr::Or(or) => write!(f, "or{or:?}"),
            ReifExpr::And(and) => write!(f, "and{and:?}"),
            ReifExpr::LinearLeq(l) => write!(f, "{l} <= 0"),
            ReifExpr::LinearEq(l) => write!(f, "{l} = 0"),
            ReifExpr::LinearNeq(l) => write!(f, "{l} != 0"),
        }
    }
}

impl ReifExpr {
    pub fn scope(&self, presence: impl Fn(Var) -> Lit) -> ValidityScope {
        match self {
            ReifExpr::Lit(l) => ValidityScope::new([presence(l.variable())], []),
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
        }
    }

    pub fn eval(&self, assignment: &Solution) -> Option<bool> {
        let prez = |var| assignment.present(var).unwrap();
        match &self {
            ReifExpr::Lit(l) => {
                if prez(l.variable()) {
                    Some(assignment.value_of(*l).unwrap())
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
            Or(lits) => And(!lits),
            And(lits) => Or(!lits),
            LinearLeq(lin) => LinearLeq(-lin + 1), // lin > 0 <=> -lin < 0 <=> -lin +1 <= 0
            LinearEq(lin) => LinearNeq(lin),
            LinearNeq(lin) => LinearEq(lin),
        }
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
