use crate::core::state::Evaluable;
use crate::model::lang::alternative::Alternative;
use crate::model::lang::linear::{LinEq, LinLeq, LinNeq};
use crate::model::lang::*;
use crate::prelude::*;
use crate::reif::{DifferenceExpression, ReifExpr};
use std::ops::Not;

use super::IVar;
use super::mul::EqMul;

pub fn leq(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> Leq {
    Leq(lhs.into(), rhs.into())
}

pub fn lt(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> Leq {
    leq(lhs.into(), rhs.into() - 1)
}

pub fn geq(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> Leq {
    leq(rhs, lhs)
}
pub fn gt(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> Leq {
    lt(rhs, lhs)
}

pub fn f_leq(lhs: impl Into<FAtom>, rhs: impl Into<FAtom>) -> Leq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.denom, rhs.denom);
    leq(lhs.num, rhs.num)
}
pub fn f_lt(lhs: impl Into<FAtom>, rhs: impl Into<FAtom>) -> Leq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.denom, rhs.denom);
    lt(lhs.num, rhs.num)
}

pub fn f_geq(lhs: impl Into<FAtom>, rhs: impl Into<FAtom>) -> Leq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.denom, rhs.denom);
    geq(lhs.num, rhs.num)
}

pub use lin_eq as eq;
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
pub fn eq_mul(lhs: impl Into<IVar>, factor1: impl Into<IVar>, factor2: impl Into<IVar>) -> EqMul {
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

#[derive(Copy, Clone, Debug)]
pub struct Leq(IAtom, IAtom);

impl Not for Leq {
    type Output = Leq;

    fn not(self) -> Self::Output {
        gt(self.0, self.1)
    }
}
impl Not for &Leq {
    type Output = Leq;

    fn not(self) -> Self::Output {
        !*self
    }
}

impl From<Leq> for ReifExpr {
    fn from(value: Leq) -> Self {
        let lhs = value.0;
        let rhs = value.1;

        // normalize, transfer the shift from right to left
        // to get: lhs <= rhs + rhs_add
        let rhs_add = rhs.shift - lhs.shift;
        let lhs: VarRef = lhs.var.into();
        let rhs: VarRef = rhs.var.into();

        // Only encode as a LEQ the patterns with two variables.
        // Other are treated either are constant (if provable as so)
        // or as literals on a single variable
        if lhs == rhs {
            // X  <= X + rhs_add   <=>  0 <= rhs_add
            (0 <= rhs_add).into()
        } else if rhs == VarRef::ZERO {
            // lhs  <= rhs_add
            Lit::leq(lhs, rhs_add).into()
        } else if lhs == VarRef::ZERO {
            // 0 <= rhs + rhs_add   <=>  -rhs_add <= rhs
            Lit::geq(rhs, -rhs_add).into()
        } else {
            ReifExpr::MaxDiff(DifferenceExpression::new(lhs, rhs, rhs_add))
        }
    }
}
