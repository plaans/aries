//! Integration test simulating a downstream crate that owns its own variable
//! type and uses the macros to generate the `std::ops` impls locally.

use aries_linexpr::{ComparisonOp, IntoLinExpr, LinExpr, VarCoeff, impl_linear_ops};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Var(pub usize);

impl_linear_ops!(Var, f64);

/// Evaluates the expression, substituting `values[var.0]` for each variable.
fn eval_f64(expr: &LinExpr<Var, f64>, values: &[f64]) -> f64 {
    expr.constant() + expr.iter().map(|(v, c)| c * values[v.0]).sum::<f64>()
}

#[test]
fn basic_ops_f64() {
    let x = Var(0);
    let y = Var(1);

    // scaling, both directions
    let t: VarCoeff<Var, f64> = x * 2.0;
    assert_eq!(t.var, x);
    assert_eq!(t.coeff, 2.0);
    let t: VarCoeff<Var, f64> = 2.0 * x;
    assert_eq!(t.var, x);

    // expression building
    let e = 2.0 * x + 3.0 * y - 1.0;
    assert_eq!(eval_f64(&e, &[1.0, 2.0]), 2.0 + 6.0 - 1.0);
    assert_eq!(e.constant(), -1.0);

    let e = x - y;
    assert_eq!(eval_f64(&e, &[5.0, 3.0]), 2.0);

    let e = -x + 4.0 * y;
    assert_eq!(eval_f64(&e, &[5.0, 1.0]), -5.0 + 4.0);

    let e = 2.0 * x + y - 0.5 * x;
    assert_eq!(eval_f64(&e, &[2.0, 1.0]), 4.0 + 1.0 - 1.0);

    // adding a bare variable to an expression
    let e = x + y;
    assert_eq!(eval_f64(&e, &[1.0, 1.0]), 2.0);

    // mixing scaled and bare variables
    let e = 2.0 * x + y;
    assert_eq!(eval_f64(&e, &[1.0, 1.0]), 3.0);

    // variable plus / minus a constant
    let e = x + 1.0;
    assert_eq!(eval_f64(&e, &[2.0]), 3.0);
    let e = x - 1.0;
    assert_eq!(eval_f64(&e, &[2.0]), 1.0);
    let e = x + 1.0 - 0.5 * x;
    assert_eq!(eval_f64(&e, &[2.0, 1.0]), 2.0 + 1.0 - 1.0);
}

#[test]
fn assign_ops_f64() {
    let x = Var(0);
    let y = Var(1);

    let mut e = LinExpr::<Var, f64>::zero();
    e += x;
    e += 2.0 * y;
    e -= 1.0;
    e -= y;
    assert_eq!(eval_f64(&e, &[1.0, 1.0]), 1.0 + 2.0 - 1.0 - 1.0);
}

#[test]
fn constraints_f64() {
    let x = Var(0);
    let y = Var(1);

    // 2x + 3y <= 4.0 : the constraint keeps the sides as they were built
    let c = (2.0 * x + 3.0 * y).leq(4.0);
    assert_eq!(c.comp_op(), ComparisonOp::Le);
    assert_eq!(eval_f64(&c.lhs, &[1.0, 1.0]), 5.0);
    assert_eq!(c.rhs.constant(), 4.0);

    // canonical form: rhs folded into the constant
    let canonical = c.canonical();
    assert_eq!(canonical.comp_op(), ComparisonOp::Le);
    assert_eq!(eval_f64(canonical.expr(), &[1.0, 1.0]), 1.0);
    assert_eq!(canonical.expr().constant(), -4.0);

    // x >= y : the rhs is a variable
    let c = x.into_linexpr().geq(y);
    assert_eq!(c.comp_op(), ComparisonOp::Ge);
    assert_eq!(eval_f64(&c.lhs, &[7.0, 3.0]), 7.0);
    assert_eq!(eval_f64(&c.rhs, &[7.0, 3.0]), 3.0);
    assert_eq!(eval_f64(c.canonical().expr(), &[7.0, 3.0]), 4.0);

    // x + y == 4 : built from assign operators
    let mut e = LinExpr::<Var, f64>::zero();
    e += x;
    e += y;
    let c = e.eq(4.0);
    assert_eq!(c.comp_op(), ComparisonOp::Eq);
    assert_eq!(eval_f64(&c.lhs, &[1.0, 2.0]), 3.0);
    assert_eq!(c.rhs.constant(), 4.0);
}

#[test]
fn canonical_form_f64() {
    let x = Var(0);
    let y = Var(1);

    // terms are sorted by variable and deduplicated in the canonical form
    let c = (2.0 * y + 2.0 * x - x).leq(1.0);
    let canonical = c.canonical();
    assert_eq!(canonical.expr().iter().collect::<Vec<_>>(), vec![(x, 1.0), (y, 2.0)]);
    assert_eq!(canonical.expr().constant(), -1.0);

    // rhs terms are folded in and deduplicated as well
    let c = x.into_linexpr().geq(2.0 * y - y + 5.0);
    let canonical = c.canonical();
    assert_eq!(canonical.expr().iter().collect::<Vec<_>>(), vec![(x, 1.0), (y, -1.0)]);
    assert_eq!(canonical.expr().constant(), -5.0);

    // the constraint itself keeps the terms as they were built
    assert_eq!(c.rhs.iter().collect::<Vec<_>>(), vec![(y, 2.0), (y, -1.0)]);
}

#[test]
fn iterable_constructors_f64() {
    let x = Var(0);
    let y = Var(1);

    // arrays, vectors and iterators of (var, coeff) pairs
    let e = LinExpr::from([(x, 1.0), (y, 2.0)]);
    assert_eq!(eval_f64(&e, &[1.0, 1.0]), 3.0);
    let e = LinExpr::from(vec![(x, 1.0), (y, 2.0)]);
    assert_eq!(eval_f64(&e, &[1.0, 1.0]), 3.0);
    let e = LinExpr::from([(x, 1.0), (y, 2.0)].iter());
    assert_eq!(eval_f64(&e, &[1.0, 1.0]), 3.0);
    let e: LinExpr<Var, f64> = vec![(x, 1.0), (y, 2.0)].into_iter().collect();
    assert_eq!(eval_f64(&e, &[1.0, 1.0]), 3.0);

    let t: VarCoeff<Var, f64> = (x, 3.0).into();
    assert_eq!(t.coeff, 3.0);
    let t: VarCoeff<Var, f64> = (&(x, 3.0)).into();
    assert_eq!(t.coeff, 3.0);
}
