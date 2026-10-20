# aries-linexpr

Type-generic building blocks for linear expressions and constraints: define your own variable
type, pick a numeric type, and build expressions with the standard arithmetic operators.

- [`VarCoeff<Var, Num>`] — a `coefficient * variable` term.
- [`LinExpr<Var, Num>`] — a constant plus a sum of scaled variables.
- [`Constraint<Var, Num>`] — `lhs {<=,=,>=} rhs`, kept exactly as constructed.
- [`CanonicalConstraint<Var, Num>`] — the same constraint with the right-hand side folded into
  the expression, whose terms are sorted by variable and deduplicated.

## Using the standard operators

The Rust orphan rules prevent this crate from implementing the standard operators on your own
variable type. Instead, the [`impl_linear_ops!`] macro generates those implementations in your
crate: invoke it in the crate that owns the variable type, at most once per variable type.

```rust
use aries_linexpr::{ComparisonOp, LinExpr, VarCoeff, impl_linear_ops};

/// Your own variable type to be used in linear expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Var(pub usize);

// implement the operator overloading traits for `Var` using `f64` as the numeric constants.
impl_linear_ops!(Var, f64);

let x = Var(0);
let y = Var(1);

// terms and expressions use the usual operators
let scaled: VarCoeff<Var, f64> = 3.0 * x;
let expr: LinExpr<Var, f64> = scaled + x;

// 2x + 3y <= 4
let constraint = (2.0 * x + 3.0 * y).leq(4.0);
assert_eq!(constraint.comp_op(), ComparisonOp::Le);

// constraints keep their as-constructed shape
assert_eq!(format!("{constraint:?}"), "2.0*Var(0) + 3.0*Var(1) <= 4.0");

// the canonical form folds the right-hand side into the expression
assert_eq!(
    format!("{:?}", constraint.canonical()),
    "2.0*Var(0) + 3.0*Var(1) - 4.0 <= 0"
);
```

Debug output is math-style: `coeff * var` products with unit coefficients, zero terms and zero
constants omitted, e.g. `2.0*x + y - 4.0 <= 1.0`.
