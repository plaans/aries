use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

use crate::{CsVec, prelude::*};

#[derive(Debug, Clone, Copy)]
pub struct ScaledVariable {
    variable: Variable,
    factor: f64,
}

/// A linear expression, composed of a constant and a sum of scaled variables.
#[derive(Debug, Clone)]
pub struct LinExpr {
    constant: f64,
    terms: Vec<ScaledVariable>,
}

impl LinExpr {
    /// An empty linear expression (0).
    pub fn zero() -> Self {
        Self {
            constant: 0.0,
            terms: Vec::new(),
        }
    }

    fn push(&mut self, term: ScaledVariable) {
        self.terms.push(term);
    }

    /// Constraint comparing this  linear expression to a constant.
    ///
    /// This is a more general form of [`Self::leq`], [`Self::geq`] and [`Self::eq`].
    pub fn into_constraint(mut self, comp_op: ComparisonOp, rhs: f64) -> Constraint {
        self -= rhs;
        Constraint { expr: self, comp_op }
    }

    /// Constraint requiring the expression to be lesser than or equal to `rhs`
    pub fn leq(self, rhs: f64) -> Constraint {
        self.into_constraint(LEQ, rhs)
    }
    /// Constraint requiring the expression to be greater than or equal to `rhs`
    pub fn geq(self, rhs: f64) -> Constraint {
        self.into_constraint(GEQ, rhs)
    }
    /// Constraint requiring the expression to be equal to `rhs`
    pub fn eq(self, rhs: f64) -> Constraint {
        self.into_constraint(EQ, rhs)
    }
}
impl From<(Variable, f64)> for ScaledVariable {
    fn from(term: (Variable, f64)) -> Self {
        ScaledVariable {
            variable: term.0,
            factor: term.1,
        }
    }
}

impl<'a> From<&'a (Variable, f64)> for ScaledVariable {
    fn from(term: &'a (Variable, f64)) -> Self {
        ScaledVariable {
            variable: term.0,
            factor: term.1,
        }
    }
}

impl<I: IntoIterator<Item = impl Into<ScaledVariable>>> From<I> for LinExpr {
    fn from(iter: I) -> Self {
        let mut expr = LinExpr::zero();
        for e in iter.into_iter() {
            expr += e.into();
        }
        expr
    }
}
impl std::iter::FromIterator<(Variable, f64)> for LinExpr {
    fn from_iter<I: IntoIterator<Item = (Variable, f64)>>(iter: I) -> Self {
        let mut expr = LinExpr::zero();
        for (var, factor) in iter {
            expr += var * factor;
        }
        expr
    }
}

impl Mul<i32> for Variable {
    type Output = ScaledVariable;

    fn mul(self, rhs: i32) -> Self::Output {
        ScaledVariable {
            variable: self,
            factor: rhs as f64,
        }
    }
}

impl Mul<Variable> for i32 {
    type Output = ScaledVariable;

    fn mul(self, rhs: Variable) -> Self::Output {
        ScaledVariable {
            variable: rhs,
            factor: self as f64,
        }
    }
}
impl Mul<f64> for Variable {
    type Output = ScaledVariable;

    fn mul(self, rhs: f64) -> Self::Output {
        ScaledVariable {
            variable: self,
            factor: rhs,
        }
    }
}

impl Mul<Variable> for f64 {
    type Output = ScaledVariable;

    fn mul(self, rhs: Variable) -> Self::Output {
        ScaledVariable {
            variable: rhs,
            factor: self,
        }
    }
}

impl Neg for ScaledVariable {
    type Output = ScaledVariable;

    fn neg(self) -> Self::Output {
        ScaledVariable {
            variable: self.variable,
            factor: -self.factor,
        }
    }
}
impl Neg for Variable {
    type Output = ScaledVariable;

    fn neg(self) -> Self::Output {
        ScaledVariable {
            variable: self,
            factor: -1.0,
        }
    }
}

impl Add<Variable> for Variable {
    type Output = LinExpr;

    fn add(self, rhs: Variable) -> Self::Output {
        LinExpr::zero() + self + rhs
    }
}
impl Add<ScaledVariable> for ScaledVariable {
    type Output = LinExpr;

    fn add(self, rhs: ScaledVariable) -> Self::Output {
        LinExpr::zero() + self + rhs
    }
}
impl Add<Variable> for ScaledVariable {
    type Output = LinExpr;

    fn add(self, rhs: Variable) -> Self::Output {
        LinExpr::zero() + self + rhs
    }
}
impl Add<ScaledVariable> for Variable {
    type Output = LinExpr;

    fn add(self, rhs: ScaledVariable) -> Self::Output {
        LinExpr::zero() + self + rhs
    }
}
impl Sub<ScaledVariable> for ScaledVariable {
    type Output = LinExpr;

    fn sub(self, rhs: ScaledVariable) -> Self::Output {
        LinExpr::zero() + self - rhs
    }
}

impl AddAssign<ScaledVariable> for LinExpr {
    fn add_assign(&mut self, rhs: ScaledVariable) {
        self.push(rhs);
    }
}

#[allow(clippy::suspicious_op_assign_impl)]
impl AddAssign<Variable> for LinExpr {
    fn add_assign(&mut self, rhs: Variable) {
        self.push(1.0 * rhs);
    }
}
#[allow(clippy::suspicious_op_assign_impl)]
impl AddAssign<f64> for LinExpr {
    fn add_assign(&mut self, rhs: f64) {
        self.constant += rhs
    }
}
impl SubAssign<ScaledVariable> for LinExpr {
    fn sub_assign(&mut self, rhs: ScaledVariable) {
        self.push(-rhs);
    }
}

#[allow(clippy::suspicious_op_assign_impl)]
impl SubAssign<Variable> for LinExpr {
    fn sub_assign(&mut self, rhs: Variable) {
        self.push(-1.0 * rhs);
    }
}
#[allow(clippy::suspicious_op_assign_impl)]
impl SubAssign<f64> for LinExpr {
    fn sub_assign(&mut self, rhs: f64) {
        self.constant -= rhs
    }
}

impl Add<ScaledVariable> for LinExpr {
    type Output = LinExpr;

    fn add(mut self, rhs: ScaledVariable) -> Self::Output {
        self.push(rhs);
        self
    }
}
#[allow(clippy::suspicious_arithmetic_impl)]
impl Add<Variable> for LinExpr {
    type Output = LinExpr;

    fn add(mut self, rhs: Variable) -> Self::Output {
        self.push(rhs * 1.0);
        self
    }
}
impl Sub<ScaledVariable> for LinExpr {
    type Output = LinExpr;

    fn sub(mut self, rhs: ScaledVariable) -> Self::Output {
        self.push(-rhs);
        self
    }
}
impl Sub<Variable> for LinExpr {
    type Output = LinExpr;

    fn sub(mut self, rhs: Variable) -> Self::Output {
        self.push(-rhs);
        self
    }
}

/// A linear constraint of the for `f_1 * v_1 + ... f_n * v_n {<=,=,>=} v`
#[derive(Debug, Clone)]
pub struct Constraint {
    pub(crate) expr: LinExpr,
    /// Comparison of the expression to 0
    pub(crate) comp_op: ComparisonOp,
}

impl Constraint {
    /// Splits the constraints into its main components
    pub fn split(self) -> (LinExpr, ComparisonOp, f64) {
        (self.expr, self.comp_op, 0.0)
    }

    pub(crate) fn decompose(&self, num_vars: usize) -> (CsVec, ComparisonOp, f64) {
        let mut vars = Vec::with_capacity(self.expr.terms.len());
        let mut coeffs = Vec::with_capacity(self.expr.terms.len());
        for t in &self.expr.terms {
            vars.push(t.variable.0);
            coeffs.push(t.factor);
        }
        (CsVec::new(num_vars, vars, coeffs), self.comp_op, -self.expr.constant)
    }
}
