use num_traits::{One, Zero};

mod ops;

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

    fn push(&mut self, term: VarCoeff<Var, Num>) {
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

    /// Constraint comparing this  linear expression to a constant.
    ///
    /// This is a more general form of [`Self::leq`], [`Self::geq`] and [`Self::eq`].
    pub fn into_constraint(self, cmp: ComparisonOp, rhs: impl Into<LinExpr<Var, Num>>) -> Constraint<Var, Num> {
        Constraint {
            lhs: self,
            cmp,
            rhs: rhs.into(),
        }
    }

    /// Constraint requiring the expression to be lesser than or equal to `rhs`
    pub fn leq(self, rhs: impl Into<LinExpr<Var, Num>>) -> Constraint<Var, Num> {
        self.into_constraint(LEQ, rhs)
    }
    /// Constraint requiring the expression to be greater than or equal to `rhs`
    pub fn geq(self, rhs: impl Into<LinExpr<Var, Num>>) -> Constraint<Var, Num> {
        self.into_constraint(GEQ, rhs)
    }
    /// Constraint requiring the expression to be equal to `rhs`
    pub fn eq(self, rhs: impl Into<LinExpr<Var, Num>>) -> Constraint<Var, Num> {
        self.into_constraint(EQ, rhs)
    }
}

pub struct Constraint<Var, Num> {
    pub lhs: LinExpr<Var, Num>,
    pub cmp: ComparisonOp,
    pub rhs: LinExpr<Var, Num>,
}
