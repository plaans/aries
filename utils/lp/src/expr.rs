use crate::{CsVec, prelude::*};

aries_linexpr::impl_linear_ops!(Variable, f64);

/// A linear expression, composed of a constant and a sum of scaled variables.
pub type LinExpr = aries_linexpr::LinExpr<Variable, f64>;

/// A linear constraint of the form `f_1 * v_1 + ... f_n * v_n {<=,=,>=} v`
pub type Constraint = aries_linexpr::Constraint<Variable, f64>;

// Compatibility with the historical API: integer literals could be used as
// scaling factors, with an implicit conversion to f64.
impl std::ops::Mul<i32> for Variable {
    type Output = aries_linexpr::VarCoeff<Variable, f64>;

    fn mul(self, rhs: i32) -> Self::Output {
        aries_linexpr::VarCoeff::new(self, rhs as f64)
    }
}

impl std::ops::Mul<Variable> for i32 {
    type Output = aries_linexpr::VarCoeff<Variable, f64>;

    fn mul(self, rhs: Variable) -> Self::Output {
        aries_linexpr::VarCoeff::new(rhs, self as f64)
    }
}

/// Decomposes the constraint into its sparse coefficient vector, its
/// comparison operator and the (negated constant) right-hand side.
pub(crate) fn decompose(c: &Constraint, num_vars: usize) -> (CsVec, ComparisonOp, f64) {
    let canonical = c.canonical();
    let expr = canonical.expr();
    let (vars, coeffs): (Vec<usize>, Vec<f64>) = expr.iter().map(|(var, coeff)| (var.0, coeff)).unzip();
    (
        CsVec::new(num_vars, vars, coeffs),
        canonical.comp_op(),
        -expr.constant(),
    )
}
