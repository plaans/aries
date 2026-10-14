use std::ops::{Add, AddAssign, Mul, Neg, Sub};

use crate::prelude::*;

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

    pub(crate) fn into_constraint(self, comp_op: ComparisonOp, rhs: f64) -> Constraint {
        let mut lhs = TermsSumSparse::empty();
        for t in self.terms {
            lhs.add(t.variable, t.factor);
        }
        Constraint {
            lhs,
            comp_op,
            rhs: rhs - self.constant,
        }
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
pub(crate) struct Constraint {
    pub(crate) lhs: TermsSumSparse,
    pub(crate) comp_op: ComparisonOp,
    pub(crate) rhs: f64,
}

/// An intermediate representation before building a sparse CsVec for representing constraints in the solver.
#[derive(Clone, Debug)]
pub(crate) struct TermsSumSparse {
    pub(crate) vars: Vec<usize>,
    pub(crate) coeffs: Vec<f64>,
}

impl TermsSumSparse {
    /// Creates an empty linear expression.
    pub fn empty() -> Self {
        Self {
            vars: vec![],
            coeffs: vec![],
        }
    }

    /// Add a single term to the linear expression.
    ///
    /// Variables can be added to an expression in any order, but adding the same variable
    /// several times is forbidden (the [`Problem::add_constraint`] method will panic).
    ///
    /// [`Problem::add_constraint`]: struct.Problem.html#method.add_constraint
    pub fn add(&mut self, var: Variable, coeff: f64) {
        self.vars.push(var.0);
        self.coeffs.push(coeff);
    }
}
