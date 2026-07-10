use crate::core::literals::Disjunction;
use crate::core::state::Evaluable;
use crate::core::{Lit, Var};
use crate::lang::ValidityScope;
use crate::model::{Label, Model};
use crate::prelude::{Conjunction, Dom, LinSum, Solution};
use std::fmt::{Debug, Formatter};
use std::ops::Not;

pub trait Reifiable<Lbl> {
    fn decompose(self, model: &mut Model<Lbl>) -> CoreExpr;
}

impl<Lbl: Label, Expr: Into<CoreExpr>> Reifiable<Lbl> for Expr {
    fn decompose(self, _: &mut Model<Lbl>) -> CoreExpr {
        self.into()
    }
}

/// The core boolean expressions supported by the solver.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub enum CoreExpr {
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

impl std::fmt::Display for CoreExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreExpr::Lit(l) => write!(f, "{l:?}"),
            CoreExpr::Or(or) => write!(f, "or{or:?}"),
            CoreExpr::And(and) => write!(f, "and{and:?}"),
            CoreExpr::LinearLeq(l) => write!(f, "{l} <= 0"),
            CoreExpr::LinearEq(l) => write!(f, "{l} = 0"),
            CoreExpr::LinearNeq(l) => write!(f, "{l} != 0"),
        }
    }
}

impl CoreExpr {
    pub fn scope(&self, presence: impl Fn(Var) -> Lit) -> ValidityScope {
        match self {
            CoreExpr::Lit(l) => ValidityScope::new([presence(l.variable())], []),
            CoreExpr::Or(literals) => ValidityScope::new(
                literals.iter().map(|l| presence(l.variable())),
                literals.iter().filter(|l| presence(l.variable()) == Lit::TRUE),
            ),
            CoreExpr::And(literals) => ValidityScope::new(
                literals.iter().map(|l| presence(l.variable())),
                literals
                    .iter()
                    .map(|l| !l)
                    .filter(|l| presence(l.variable()) == Lit::TRUE),
            ),
            CoreExpr::LinearLeq(lin) | CoreExpr::LinearEq(lin) | CoreExpr::LinearNeq(lin) => {
                ValidityScope::new(lin.variables().map(presence), [])
            }
        }
    }

    pub fn eval(&self, assignment: &Solution) -> Option<bool> {
        let prez = |var| assignment.present(var).unwrap();
        match &self {
            CoreExpr::Lit(l) => {
                if prez(l.variable()) {
                    Some(assignment.value_of(*l).unwrap())
                } else {
                    None
                }
            }
            CoreExpr::Or(lits) => {
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
            CoreExpr::And(_) => (!self.clone()).eval(assignment).map(|value| !value),
            CoreExpr::LinearLeq(lin) => lin.evaluate(assignment).map(|value| value <= 0),
            CoreExpr::LinearEq(lin) => lin.evaluate(assignment).map(|value| value == 0),
            CoreExpr::LinearNeq(lin) => lin.evaluate(assignment).map(|value| value != 0),
        }
    }
}

impl From<bool> for CoreExpr {
    fn from(value: bool) -> Self {
        CoreExpr::Lit(value.into())
    }
}

impl From<Lit> for CoreExpr {
    fn from(value: Lit) -> Self {
        CoreExpr::Lit(value)
    }
}

impl From<Disjunction> for CoreExpr {
    fn from(value: Disjunction) -> Self {
        if value.is_tautology() {
            CoreExpr::Lit(Lit::TRUE)
        } else if value.literals().is_empty() {
            CoreExpr::Lit(Lit::FALSE)
        } else if value.literals().len() == 1 {
            CoreExpr::Lit(*value.literals().first().unwrap())
        } else {
            CoreExpr::Or(value)
        }
    }
}
impl From<Conjunction> for CoreExpr {
    fn from(value: Conjunction) -> Self {
        // go through a disjunction to reuse the simplications
        // this may be a bit wasteful and coudl beneift from a direct implementation
        // (but conjunctions are pretty rare in most problems)
        !CoreExpr::from(!value)
    }
}

impl Not for CoreExpr {
    type Output = Self;

    fn not(self) -> Self::Output {
        use CoreExpr::*;
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
    use crate::{core::Lit, reif::CoreExpr};

    #[test]
    fn test_reif_expr_size() {
        if std::mem::size_of::<Lit>() == 8 {
            assert_eq!(std::mem::size_of::<CoreExpr>(), 40)
        }
    }
}
