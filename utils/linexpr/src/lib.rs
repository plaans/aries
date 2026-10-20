use std::ops::AddAssign;

use num_traits::{One, Zero};

// Re-exported so that the `impl_linear_ops!` macro can reference `num_traits`
// items (e.g. `One`, `Zero`) from the caller's crate without requiring the
// caller to add `num-traits` to its own dependencies.
#[doc(hidden)]
pub use num_traits;

mod ops;

pub use ops::IntoLinExpr;
use smallvec::{SmallVec, smallvec};

/// An operator specifying the relation between left-hand and right-hand sides of the constraint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComparisonOp {
    /// The == operator (equal to)
    Eq,
    /// The <= operator (less than or equal to)
    Le,
    /// The >= operator (greater than or equal to)
    Ge,
}

/// `<=` Comparison operator.
pub const LEQ: ComparisonOp = ComparisonOp::Le;

/// `>=` Comparison operator.
pub const GEQ: ComparisonOp = ComparisonOp::Ge;

/// `=` Comparison operator.
pub const EQ: ComparisonOp = ComparisonOp::Eq;

/// A `var * coeff` term.
#[derive(Debug, Clone, Copy)]
pub struct VarCoeff<Var, Num> {
    /// Variable of this term.
    pub var: Var,
    /// Coefficient of the variable.
    pub coeff: Num,
}

impl<Var, Num> VarCoeff<Var, Num> {
    pub fn new(var: Var, coeff: Num) -> Self {
        Self { var, coeff }
    }

    pub fn from_variable(var: Var) -> Self
    where
        Num: One,
    {
        Self::new(var, Num::one())
    }

    /// Splits this term into its `(variable, coefficient)` parts.
    pub fn into_parts(self) -> (Var, Num) {
        (self.var, self.coeff)
    }
}

/// A linear expression, composed of a constant and a sum of scaled variables.
#[derive(Debug, Clone)]
pub struct LinExpr<Var, Num> {
    constant: Num,
    terms: SmallVec<[VarCoeff<Var, Num>; 2]>,
}

impl<Var, Num> LinExpr<Var, Num> {
    /// An empty linear expression (0).
    pub fn zero() -> Self
    where
        Num: Zero,
    {
        Self {
            constant: Num::zero(),
            terms: smallvec![],
        }
    }

    /// An expression containing only the given constant.
    pub fn from_constant(constant: Num) -> Self {
        Self {
            constant,
            terms: smallvec![],
        }
    }

    /// Adds a term to this expression.
    pub fn push(&mut self, term: VarCoeff<Var, Num>) {
        self.terms.push(term);
    }

    /// The constant term of this expression.
    pub fn constant(&self) -> Num
    where
        Num: Copy,
    {
        self.constant
    }

    /// All variable terms in this expressions (excluding the constant).
    pub fn terms(&self) -> &[VarCoeff<Var, Num>] {
        &self.terms
    }

    /// The `(variable, factor)` terms of this expression.
    pub fn iter(&self) -> impl Iterator<Item = (Var, Num)> + '_
    where
        Var: Copy,
        Num: Copy,
    {
        self.terms.iter().map(|t| (t.var, t.coeff))
    }

    /// Constraint comparing this  linear expression to `rhs`.
    ///
    /// This is a more general form of [`Self::leq`], [`Self::geq`] and [`Self::eq`].
    ///
    /// The constraint keeps the left-hand and right-hand sides as they were at
    /// construction. Use [`Constraint::canonical`] to get the folded representation.
    pub fn into_constraint(self, cmp: ComparisonOp, rhs: impl IntoLinExpr<Var, Num>) -> Constraint<Var, Num> {
        Constraint {
            lhs: self,
            cmp,
            rhs: rhs.into_linexpr(),
        }
    }

    /// Constraint requiring the expression to be lesser than or equal to `rhs`
    pub fn leq(self, rhs: impl IntoLinExpr<Var, Num>) -> Constraint<Var, Num> {
        self.into_constraint(LEQ, rhs)
    }
    /// Constraint requiring the expression to be greater than or equal to `rhs`
    pub fn geq(self, rhs: impl IntoLinExpr<Var, Num>) -> Constraint<Var, Num> {
        self.into_constraint(GEQ, rhs)
    }
    /// Constraint requiring the expression to be equal to `rhs`
    pub fn eq(self, rhs: impl IntoLinExpr<Var, Num>) -> Constraint<Var, Num> {
        self.into_constraint(EQ, rhs)
    }

    /// Normalizes the expression to its canonical form, where terms are sorted and deduplicated.
    pub fn simplify(&mut self)
    where
        Var: Ord + Copy,
        Num: Zero + AddAssign + Copy,
    {
        self.terms.sort_unstable_by_key(|sv| sv.var);
        self.terms.dedup_by(|second, first| {
            if first.var == second.var {
                // same variables, merge
                first.coeff += second.coeff;
                true // remove second
            } else {
                false // different terms, don't merge
            }
        });
        self.terms.retain(|sv| !sv.coeff.is_zero());
    }
}

/// A linear constraint of the form `lhs {<=,=,>=} rhs`.
///
/// The two sides are kept as they were at construction, which is convenient
/// for debugging and presentation. Use [`Self::canonical`] to obtain the
/// equivalent [`CanonicalConstraint`].
#[derive(Debug, Clone)]
pub struct Constraint<Var, Num> {
    /// The left-hand side of the constraint.
    pub lhs: LinExpr<Var, Num>,
    /// Comparison of the left-hand and right-hand sides.
    pub cmp: ComparisonOp,
    /// The right-hand side of the constraint.
    pub rhs: LinExpr<Var, Num>,
}

impl<Var, Num> Constraint<Var, Num> {
    /// The comparison operator between the left-hand and right-hand sides.
    pub fn comp_op(&self) -> ComparisonOp {
        self.cmp
    }

    /// The equivalent constraint in canonical form: the right-hand side is
    /// folded into the constant of the expression, whose terms are sorted by
    /// variable and deduplicated.
    pub fn canonical(&self) -> CanonicalConstraint<Var, Num>
    where
        Var: Ord + Copy,
        Num: Copy + Zero + std::ops::AddAssign + std::ops::Neg<Output = Num>,
    {
        CanonicalConstraint::new(self.lhs.clone(), self.cmp, &self.rhs)
    }
}

/// A linear constraint of the form `f_1 * v_1 + ... + f_n * v_n {<=,=,>=} 0` in
/// canonical form: the right-hand side has been folded into the constant of
/// the expression, whose terms are sorted by variable and deduplicated.
///
/// Obtained from a [`Constraint`] with [`Constraint::canonical`].
#[derive(Debug, Clone)]
pub struct CanonicalConstraint<Var, Num> {
    /// The expression, compared to zero.
    expr: LinExpr<Var, Num>,
    /// Comparison of the expression to zero.
    cmp: ComparisonOp,
}

impl<Var, Num> CanonicalConstraint<Var, Num> {
    fn new(mut lhs: LinExpr<Var, Num>, cmp: ComparisonOp, rhs: &LinExpr<Var, Num>) -> Self
    where
        Var: Ord + Copy,
        Num: Copy + Zero + std::ops::AddAssign + std::ops::Neg<Output = Num>,
    {
        // retract the RHS from the LHS
        lhs.constant += -rhs.constant;

        lhs.terms.reserve(rhs.terms.len());
        for (var, coeff) in rhs.iter() {
            lhs.terms.push(VarCoeff::new(var, -coeff));
        }

        // put in canonical form
        lhs.simplify();

        Self { expr: lhs, cmp }
    }

    /// The expression that this constraint compares to zero.
    ///
    /// Guaranteed to be in simplified form.
    pub fn expr(&self) -> &LinExpr<Var, Num> {
        &self.expr
    }

    /// The comparison operator applied between the expression and zero.
    pub fn comp_op(&self) -> ComparisonOp {
        self.cmp
    }
}
