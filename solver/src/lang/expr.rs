//! This module provides high-level methods to construct boolean and integer expressions.
//!
//! These are all re-exported in `[crate::prelude`].

use itertools::Itertools;

use crate::core::views::Dom;
use crate::lang::constraints::{AllDifferent, Alternative, EqMax};
use crate::lang::linear::{LinEq, LinLeq, LinNeq};
use crate::lang::mul::EqMul;
use crate::lang::*;
use crate::prelude::*;

// CAREFUL: any public item here will be re-exported in the `prelude`

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

/// Requires that all integer expressions have a different value, ignoring those that are absent.
pub fn all_different<T: Into<LinSum>>(vars: impl IntoIterator<Item = T>) -> AllDifferent {
    AllDifferent::new(vars.into_iter().map(|var| var.into()))
}

/// The `alternative` constraint, that imposes that exactly one of the `alternative` element will be selected to decide the `main` value.
///
/// See: [`Alternative`]
pub fn alternative<T: Into<IAtom>, TAlt: Into<IAtom>>(
    main: T,
    alternatives: impl IntoIterator<Item = TAlt>,
) -> Alternative {
    Alternative::new(main.into(), alternatives.into_iter().map(|a| a.into()).collect_vec())
}

/// Creates a new expression that is true iff `lhs = factor1 * factor2`
pub fn eq_mul(lhs: impl Into<Var>, factor1: impl Into<Var>, factor2: impl Into<Var>) -> EqMul {
    EqMul::new(lhs.into(), factor1.into(), factor2.into())
}

/// Requires that the value of the LHS is equal to maximum value of the RHS elements.
///
/// If some variables are optional, it will require that, if the LHS is present, then at least one element
/// of the RHS is present as well.
pub fn eq_max<Var>(lhs: Var, rhs: impl IntoIterator<Item = Var>) -> EqMax<Var> {
    EqMax::new(lhs, rhs.into_iter().collect_vec())
}

/// Requires that the value of the LHS is equal to maximum value of the RHS elements.
///
/// If some variables are optional, it will require that, if the LHS is present, then at least one element
/// of the RHS is present as well.
///
/// Note that constraint exploits a reformulation into an equivalent `eq_max` expression.
pub fn eq_min<Var, NegVar>(lhs: Var, rhs: impl IntoIterator<Item = Var>) -> EqMax<NegVar>
where
    Var: std::ops::Neg<Output = NegVar>,
{
    eq_max(-lhs, rhs.into_iter().map(|e| -e))
}

/// Transforms a boolean into an integer expression where `1` represents `true` and `0` represents false.
///
/// In most cases, this will reuse the variable underlying the literal but it may in some case require the introduction
/// of an auxiliary variable.
pub fn bool2int<Ctx: Store + Dom>(b: Lit, model: &mut Ctx) -> LinTerm {
    let is_zero_one = model.bounds(b.variable()) == (0, 1);
    if model.entails(b) {
        1.into()
    } else if model.entails(!b) {
        0.into()
    } else if is_zero_one && b == b.variable().geq(1) {
        b.variable().into()
    } else if is_zero_one && b == b.variable().leq(0) {
        1 - b.variable()
    } else {
        let binary_var = model.new_optional_var(0, 1, model.presence(b));
        implies(binary_var.geq(1), b).enforce(model);
        implies(b, binary_var.geq(1)).enforce(model);
        LinTerm::from(binary_var)
    }
}
