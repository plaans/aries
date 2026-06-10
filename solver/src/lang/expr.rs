use crate::lang::alternative::Alternative;
use crate::lang::linear::{LinEq, LinLeq, LinNeq};
use crate::lang::*;
use crate::prelude::*;

use super::mul::EqMul;

pub fn eq(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinEq {
    lhs.into().eq(rhs)
}
pub fn neq(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinNeq {
    lhs.into().neq(rhs)
}
pub fn leq(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinLeq {
    lhs.into().leq(rhs)
}
pub fn geq(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinLeq {
    lhs.into().geq(rhs)
}
pub fn lt(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinLeq {
    lhs.into().lt(rhs)
}
pub fn gt(lhs: impl Into<LinSum>, rhs: impl Into<LinSum>) -> LinLeq {
    lhs.into().gt(rhs)
}

pub fn or(disjuncts: impl Into<Disjunction>) -> Disjunction {
    disjuncts.into()
}
pub fn and(conjuncts: impl Into<Conjunction>) -> Conjunction {
    conjuncts.into()
}
pub fn implies(a: impl Into<Lit>, b: impl Into<Lit>) -> Disjunction {
    or([!a.into(), b.into()])
}

/// Creates a new expression that is true iff `lhs = factor1 * factor2`
pub fn eq_mul(lhs: impl Into<Var>, factor1: impl Into<Var>, factor2: impl Into<Var>) -> EqMul {
    EqMul::new(lhs.into(), factor1.into(), factor2.into())
}

pub fn alternative<T: Into<IAtom>>(main: impl Into<IAtom>, alternatives: impl IntoIterator<Item = T>) -> Alternative {
    Alternative::new(main, alternatives)
}

// pub type Or = Disjunction;

// pub type And = Conjunction;
