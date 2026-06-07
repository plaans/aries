use crate::core::state::Evaluable;
use crate::lang::alternative::Alternative;
use crate::lang::linear::{LinEq, LinLeq, LinNeq};
use crate::lang::*;
use crate::prelude::*;

use super::mul::EqMul;

// TODO: use a single term (for backward compatibility currently)
pub use lin_eq as eq;
pub use lin_geq as geq;
pub use lin_gt as gt;
pub use lin_leq as leq;
pub use lin_lt as lt;
pub use lin_neq as neq;

pub fn lin_eq(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinEq {
    lhs.into().eq(rhs)
}
pub fn lin_neq(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinNeq {
    lhs.into().neq(rhs)
}
pub fn lin_leq(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinLeq {
    lhs.into().leq(rhs)
}
pub fn lin_geq(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinLeq {
    lhs.into().geq(rhs)
}
pub fn lin_lt(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinLeq {
    lhs.into().lt(rhs)
}
pub fn lin_gt(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinLeq {
    lhs.into().gt(rhs)
}

pub fn or(disjuncts: impl Into<Disjunction>) -> Or {
    disjuncts.into()
}
pub fn and(conjuncts: impl Into<Conjunction>) -> And {
    conjuncts.into()
}
pub fn implies(a: impl Into<Lit>, b: impl Into<Lit>) -> Or {
    or([!a.into(), b.into()])
}

/// Creates a new expression that is true iff `lhs = factor1 * factor2`
pub fn eq_mul(lhs: impl Into<Var>, factor1: impl Into<Var>, factor2: impl Into<Var>) -> EqMul {
    EqMul::new(lhs.into(), factor1.into(), factor2.into())
}

pub fn alternative<T: Into<IAtom>>(main: impl Into<IAtom>, alternatives: impl IntoIterator<Item = T>) -> Alternative {
    Alternative::new(main, alternatives)
}

pub type Or = Disjunction;

impl Evaluable for Disjunction {
    type Value = bool;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        if self.iter().any(|l| l.evaluate(solution) == Some(true)) {
            Some(true)
        } else if self.iter().any(|l| l.evaluate(solution).is_none()) {
            None
        } else {
            Some(false)
        }
    }
}

pub type And = Conjunction;
