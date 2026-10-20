use num_traits::{One, Zero};

// Re-exported so that the `impl_linear_ops!` macro can reference `num_traits`
// items (e.g. `One`, `Zero`) from the caller's crate without requiring the
// caller to add `num-traits` to its own dependencies.
pub use num_traits;

mod ops;

pub use ops::IntoLinExpr;

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

#[derive(Debug, Clone, Copy)]
pub struct VarCoeff<Var, Num> {
    var: Var,
    coeff: Num,
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

    /// The variable of this term.
    pub fn var(&self) -> &Var {
        &self.var
    }

    /// The coefficient of this term.
    pub fn coeff(&self) -> &Num {
        &self.coeff
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
    terms: Vec<VarCoeff<Var, Num>>,
}

impl<Var, Num> LinExpr<Var, Num> {
    /// An empty linear expression (0).
    pub fn zero() -> Self
    where
        Num: Zero,
    {
        Self {
            constant: Num::zero(),
            terms: Vec::new(),
        }
    }

    /// An expression containing only the given constant.
    pub fn from_constant(constant: Num) -> Self {
        Self {
            constant,
            terms: Vec::new(),
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
    /// construction. Use [`Self::canonical`] to get the folded representation.
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
        let mut constant = self.lhs.constant();
        constant += -self.rhs.constant();

        let mut terms = Vec::with_capacity(self.lhs.terms.len() + self.rhs.terms.len());
        for (var, coeff) in self.lhs.iter() {
            terms.push(VarCoeff::new(var, coeff));
        }
        for (var, coeff) in self.rhs.iter() {
            terms.push(VarCoeff::new(var, -coeff));
        }
        terms.sort_by_key(|term| term.var);

        let mut merged: Vec<VarCoeff<Var, Num>> = Vec::with_capacity(terms.len());
        for term in terms {
            match merged.last_mut() {
                Some(last) if last.var == term.var => last.coeff += term.coeff,
                _ => merged.push(term),
            }
        }

        CanonicalConstraint {
            expr: LinExpr {
                constant,
                terms: merged,
            },
            cmp: self.cmp,
        }
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
    /// The expression that this constraint compares to zero.
    pub fn expr(&self) -> &LinExpr<Var, Num> {
        &self.expr
    }

    /// The comparison operator applied between the expression and zero.
    pub fn comp_op(&self) -> ComparisonOp {
        self.cmp
    }
}
