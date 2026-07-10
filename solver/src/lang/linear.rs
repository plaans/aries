use num_integer::{div_ceil, div_floor};
use smallvec::SmallVec;

use crate::core::state::Evaluable;
use crate::core::views::{Boundable, Dom, Term, VarView};
use crate::core::{IntCst, Lit, LongCst, SignedVar, Var, cst_long_to_int_clamped};
use crate::lang::{BoolExpr, IntExpr, Store};
use crate::prelude::Conjunction;
use crate::reif::CoreExpr;
use std::fmt::{Debug, Display};

/* ========================================================================== */
/*                               ScaledVar                                    */
/* ========================================================================== */

/// A term of the form `a * X` where `X` is an integer variable and `a` is a integer constant.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct ScaledVar {
    /// Variable `X` to which the factor is applied
    ///
    /// Note that the order is important so that `Ord` considers first the variable when ordering a list.
    /// This is relied upon when normalizing a linear sum.
    pub var: Var,
    /// Factor `a` by which the variable is multiplied.
    pub factor: IntCst,
}

impl ScaledVar {
    /// Constant always equal to zero.
    pub const ZERO: ScaledVar = ScaledVar::new(Var::ZERO, 0);
    pub const fn new(var: Var, factor: IntCst) -> Self {
        Self { var, factor }
    }

    /// Returns true if the term is always equal to zero.
    pub fn is_zero(&self) -> bool {
        self.factor == 0 || self.var == Var::ZERO
    }
}

impl Display for ScaledVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.factor {
            _ if self.is_zero() => write!(f, "0"),
            1 => write!(f, "{:?}", self.var),
            -1 => write!(f, "-{:?}", self.var),
            _ => write!(f, "{}*{:?}", self.factor, self.var),
        }
    }
}

impl std::fmt::Debug for ScaledVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

/// A normalized version of [`ScaledVar`] that make operating on bounds more efficient and straightfoward.
struct ScaledVarImpl {
    /// Factor, always strictly positive
    factor: IntCst,
    /// A signed variable that catpures the original sign of the factor and is [`SignedVar::ZERO`]
    /// if the the original factor was zero.
    var: SignedVar,
}
impl From<ScaledVar> for ScaledVarImpl {
    fn from(value: ScaledVar) -> Self {
        match value.factor.cmp(&0) {
            std::cmp::Ordering::Less => ScaledVarImpl {
                factor: value.factor.abs(),
                var: SignedVar::minus(value.var),
            },
            std::cmp::Ordering::Equal => ScaledVarImpl {
                factor: 1,
                var: SignedVar::ZERO,
            },
            std::cmp::Ordering::Greater => ScaledVarImpl {
                factor: value.factor,
                var: SignedVar::plus(value.var),
            },
        }
    }
}

impl VarView for ScaledVarImpl {
    type Value = IntCst;

    fn upper_bound(&self, dom: impl Dom) -> Self::Value {
        debug_assert!(self.factor > 0);
        dom._upper_bound(self.var) * self.factor
    }

    fn lower_bound(&self, dom: impl Dom) -> Self::Value {
        debug_assert!(self.factor > 0);
        dom._lower_bound(self.var) * self.factor
    }
}

impl Boundable for ScaledVarImpl {
    type Value = IntCst;

    fn leq(&self, ub: Self::Value) -> Lit {
        debug_assert!(self.factor > 0);
        // a*X <= ub
        // X <= ub/a   (floor gets us the first integer value below)
        self.var.leq(div_floor(ub, self.factor))
    }

    fn geq(&self, lb: Self::Value) -> Lit {
        debug_assert!(self.factor > 0);
        // a*X >= lb
        // X >= lb/a
        self.var.geq(div_ceil(lb, self.factor))
    }
}

impl VarView for ScaledVar {
    type Value = IntCst; // TODO: this should be LongCst to avoid possible overflows

    fn upper_bound(&self, dom: impl crate::core::views::Dom) -> Self::Value {
        ScaledVarImpl::from(*self).upper_bound(dom)
    }

    fn lower_bound(&self, dom: impl crate::core::views::Dom) -> Self::Value {
        ScaledVarImpl::from(*self).lower_bound(dom)
    }
}

impl Boundable for ScaledVar {
    type Value = IntCst;

    fn leq(&self, ub: Self::Value) -> Lit {
        ScaledVarImpl::from(*self).leq(ub)
    }

    fn geq(&self, lb: Self::Value) -> Lit {
        ScaledVarImpl::from(*self).geq(lb)
    }
}

impl Term for ScaledVar {
    fn variable(self) -> Var {
        self.var
    }
}

/// A term of the form `a * X + b` where `X` is an integer variable and `a` and `b` are integer constants.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinTerm {
    pub scaled_var: ScaledVar,
    pub constant: IntCst,
}

impl LinTerm {
    pub const fn new(scaled_var: ScaledVar, constant: IntCst) -> Self {
        Self { scaled_var, constant }
    }
    pub const fn int_cst(constant: IntCst) -> Self {
        Self::new(ScaledVar::ZERO, constant)
    }

    pub const ZERO: Self = Self::int_cst(0);
    pub const TRUE: Self = Self::int_cst(1);
    pub const FALSE: Self = Self::int_cst(0);

    pub fn eq<Rhs: Into<LinSum>>(&self, other: Rhs) -> LinEq {
        LinSum::from(*self).eq(other.into())
    }
}

impl Debug for LinTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.scaled_var.is_zero() {
            return write!(f, "{}", self.constant);
        }
        write!(f, "{:?}", self.scaled_var)?;
        if self.constant > 0 {
            write!(f, " + {}", self.constant)?;
        } else if self.constant < 0 {
            write!(f, " - {}", self.constant.abs())?;
        }
        Ok(())
    }
}

impl VarView for LinTerm {
    type Value = IntCst;

    fn upper_bound(&self, dom: impl crate::core::views::Dom) -> Self::Value {
        self.scaled_var.upper_bound(dom) + self.constant
    }

    fn lower_bound(&self, dom: impl crate::core::views::Dom) -> Self::Value {
        self.scaled_var.lower_bound(dom) + self.constant
    }
}

impl Boundable for LinTerm {
    type Value = IntCst;

    fn leq(&self, ub: Self::Value) -> Lit {
        // a*X + b <= ub
        // a*X <= ub -b
        self.scaled_var.leq(ub - self.constant)
    }

    fn geq(&self, lb: Self::Value) -> Lit {
        // a*X + b >= lb
        // a*X >= lb -b
        self.scaled_var.geq(lb - self.constant)
    }
}
impl Term for LinTerm {
    fn variable(self) -> Var {
        self.scaled_var.variable()
    }
}

/// A linear sum representation, of the form `c + c1 * X1 + c2 * X2 + ...`.
///
/// The linear sum is *always* in a normalized form:
///  - a variable may only occur once
///  - no variables appear with a factor of 0
///  - the variables [`Var::ZERO`] and [`Var::ONE`] may not appear
///  - the terms are sorted (with the variables as sorting keys)
///
/// The linear sum is defined when all variables in it are present (see: [`LinSum::conj_scope`]).
///
/// ## Performance
///
/// The type guarantees no allocation for sum with at most two variables.
///
/// Maintaining the invariant that the sum is in normal form is in
/// `O(n*log(n))` where `n` is the number of terms in the sum.
/// This can make constructed a sum iteratively by adding element one by one
/// expensive for large sum (you pay the cost at each addition).
///
/// The typical work around would be to do batched addition ([`LinSum::new`] accepts an iterator )
/// but we may consider having a builder (which fits more naturally in existing code) if that becomes
/// a recurring issue (similar to `ConjunctionBuilder`)
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LinSum {
    /// All scaled variables appearing in the sum (`ci +Xi`).
    ///
    /// IMPORTANT: must remain private, to ensure that invariants always hold
    vars: SmallVec<[ScaledVar; 2]>,
    /// Constant term in the sum (`c`)
    constant: IntCst,
}

impl LinSum {
    pub fn cst(constant: IntCst) -> Self {
        Self {
            vars: SmallVec::new(),
            constant,
        }
    }
    pub fn zero() -> Self {
        Self::cst(0)
    }
    pub fn new(constant: IntCst, vars: impl IntoIterator<Item = ScaledVar>) -> Self {
        let mut out = Self {
            vars: SmallVec::from_iter(vars),
            constant,
        };
        out.simplify();
        out
    }
    pub fn eq<Rhs: Into<LinSum>>(self, other: Rhs) -> LinEq {
        LinEq(self - other)
    }
    pub fn neq<Rhs: Into<LinSum>>(self, other: Rhs) -> LinNeq {
        LinNeq(self - other)
    }
    pub fn leq<Rhs: Into<LinSum>>(self, upper_bound: Rhs) -> LinLeq {
        LinLeq(self - upper_bound)
    }
    pub fn geq<Rhs: Into<LinSum>>(self, lower_bound: Rhs) -> LinLeq {
        LinLeq(lower_bound.into() - self)
    }
    pub fn lt<Rhs: Into<LinSum>>(self, upper_bound: Rhs) -> LinLeq {
        // a < b <=> a - b < 0 <=> a -b <= -1
        (self - upper_bound).leq(-1)
    }
    pub fn gt<Rhs: Into<LinSum>>(self, lower_bound: Rhs) -> LinLeq {
        // a > b <=> b < a
        lower_bound.into().lt(self)
    }

    /// Returns the conjunction of all presence literals of variables appearing in the sum.
    pub fn conj_scope(&self, dom: impl Dom) -> Conjunction {
        Conjunction::from_iter(self.vars.iter().map(|sv| dom._presence(sv.var)))
    }

    /// Simplify the terms of the expression, into a normalized expression that satisfies
    /// all our invariants.
    ///
    /// Note that it should be an invariant that the `LinSum` is always in its normal form.
    ///
    /// This is private because no-one outside this module should be able to hold a `LinSum` that is not normalized already.
    fn simplify(&mut self) {
        self.vars.sort_unstable_by_key(|sv| sv.var);
        self.vars.dedup_by(|second, first| {
            if first.var == second.var {
                // same variables, merge
                first.factor += second.factor;
                true // remove second
            } else {
                false // different vars, don't merge
            }
        });
        self.vars.retain(|sv| {
            if sv.is_zero() {
                false
            } else if sv.var == Var::ONE {
                self.constant += sv.factor;
                false
            } else {
                true
            }
        });
    }

    pub fn constant(&self) -> IntCst {
        self.constant
    }

    /// Returns an iterator over all non-constant tems.
    pub fn terms(&self) -> impl Iterator<Item = ScaledVar> + '_ {
        self.vars.iter().copied()
    }

    pub(crate) fn terms_slice(&self) -> &[ScaledVar] {
        &self.vars
    }

    /// Returns an iterator over all variables appearing in the sum (without their factor).
    /// Variables are guaranteed to appear at most once.
    pub fn variables(&self) -> impl Iterator<Item = Var> + '_ {
        self.vars.iter().map(|sv| sv.var)
    }

    /// Extract the different parts of the expression, return `None` if th linear sum has a different number of variable terms.
    pub fn extract<const N_VARS: usize>(&self) -> Option<(IntCst, [(IntCst, Var); N_VARS])> {
        self.vars
            .as_array()
            .map(|vars| (self.constant, vars.map(|sv| (sv.factor, sv.var))))
    }

    /// Returns the ith variable term
    pub fn get_var(&self, var_index: usize) -> ScaledVar {
        self.vars[var_index]
    }

    // Implementation of operations that require private access
    // These are publicly exposed as implmentations for `std::ops:*` traits.

    pub(super) fn add_assign_impl(&mut self, other: Self) {
        self.vars.extend_from_slice(&other.vars);
        self.constant += other.constant;
        self.simplify();
    }

    pub(super) fn mul_assign_impl(&mut self, factor: IntCst) {
        self.constant *= factor;
        self.vars.iter_mut().for_each(|sv| sv.factor *= factor);
        self.simplify();
    }
}

impl<Ctx: Store> IntExpr<Ctx> for LinSum {
    fn enforce_eq_if(&self, implicant: Lit, variable: LinTerm, ctx: &mut Ctx) {
        self.clone().eq(variable).enforce_if(implicant, ctx);
    }
}

impl Evaluable for LinSum {
    type Value = IntCst;

    fn evaluate(&self, solution: &crate::prelude::Solution) -> Option<Self::Value> {
        let mut value = self.constant as LongCst;
        for var in &self.vars {
            value += (var.factor as LongCst) * (solution.eval(var.var)? as LongCst)
        }
        Some(cst_long_to_int_clamped(value))
    }
}

impl std::fmt::Display for LinSum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, e) in self.vars.iter().enumerate() {
            if e.factor < 0 {
                if i != 0 {
                    write!(f, " - ")?;
                } else {
                    write!(f, "-")?;
                }
            } else if i > 0 {
                write!(f, " + ")?;
            }
            if e.factor.abs() != 1 {
                write!(f, "{}", e.factor.abs())?;
            }
            if e.factor.abs() != 1 && e.var != Var::ONE {
                write!(f, "*")?;
            }
            if e.var != Var::ONE {
                write!(f, "{:?}", e.var)?;
            } else if e.factor.abs() == 1 {
                write!(f, "1")?;
            }
        }
        Ok(())
    }
}

impl From<&LinSum> for LinSum {
    fn from(value: &LinSum) -> Self {
        value.clone()
    }
}

/// A linear inequality over integer variables.
///
/// The expression is true iff the linear sum is lesser than or equal to zero.
#[derive(Debug, Clone)]
pub struct LinLeq(LinSum);

/// A linear equality over integer variables.
///
/// The expression is true iff the linear sum is equal to zero.
#[derive(Debug, Clone)]
pub struct LinEq(LinSum);

/// A linear disequality over integer variables.
///
/// The expression is true iff the linear sum is *not* equal to zero.
#[derive(Debug, Clone)]
pub struct LinNeq(LinSum);

impl std::ops::Not for LinEq {
    type Output = LinNeq;

    fn not(self) -> Self::Output {
        LinNeq(self.0)
    }
}

impl std::ops::Not for LinNeq {
    type Output = LinEq;

    fn not(self) -> Self::Output {
        LinEq(self.0)
    }
}

impl std::ops::Not for LinLeq {
    type Output = LinLeq;

    fn not(self) -> Self::Output {
        self.0.geq(1)
    }
}
impl std::ops::Not for &LinEq {
    type Output = LinNeq;

    fn not(self) -> Self::Output {
        LinNeq(self.0.clone())
    }
}
impl std::ops::Not for &LinNeq {
    type Output = LinEq;

    fn not(self) -> Self::Output {
        LinEq(self.0.clone())
    }
}

impl std::ops::Not for &LinLeq {
    type Output = LinLeq;

    fn not(self) -> Self::Output {
        self.0.clone().geq(1)
    }
}

impl From<LinLeq> for CoreExpr {
    fn from(value: LinLeq) -> Self {
        CoreExpr::LinearLeq(value.0)
    }
}
impl From<LinEq> for CoreExpr {
    fn from(value: LinEq) -> Self {
        CoreExpr::LinearEq(value.0)
    }
}
impl From<LinNeq> for CoreExpr {
    fn from(value: LinNeq) -> Self {
        CoreExpr::LinearNeq(value.0)
    }
}

/* ========================================================================== */
/*                                 Unit Tests                                 */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_simplify_linear_sum() {
        // Terms should be grouped by (lit, variable)
        // Terms with null `factor` or `variable` equals to Var::ZERO should be filtered
        // Terms with null `variable` and `literal` equals to Lit::TRUE  should be grouped into the upper bound
        let var0 = Var::ZERO;
        let var1 = Var::from_u32(5);
        let var2 = Var::from_u32(6);
        let var3 = Var::from_u32(7);

        let item = |factor: IntCst, var: Var| ScaledVar { var, factor };

        let obj = LinSum::new(
            -5,
            vec![
                // Variable terms with zero variable, should be filtered
                item(30, var0),
                item(35, var0),
                item(40, var0),
                item(45, var0),
                // Variable terms with null factor, should be filtered
                item(0, var1),
                item(0, var1),
                item(0, var1),
                item(0, var1),
                // Other variable terms no specificities, should be grouped by lit
                item(-1, var2),
                item(50, var1),
                item(55, var1),
                item(60, var2),
                item(65, var2),
                item(-5, var2),
                item(5, var1),
                // var3 cancels out and should disappear
                item(5, var3),
                item(-5, var3),
            ],
        );

        assert_eq!(obj.constant(), -5);

        // Terms could have been reorganized
        let expected_sum: HashSet<_> = [
            // Other variable terms no specificities, should be grouped by lit
            item(110, var1),
            item(119, var2),
        ]
        .into_iter()
        .collect();
        assert_eq!(expected_sum, obj.terms().collect());
    }
}
