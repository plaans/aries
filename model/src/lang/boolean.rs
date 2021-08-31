use crate::expressions::ExprHandle;

use crate::bounds::Lit;
use crate::lang::{ConversionError, IVar, VarRef};
use std::cmp::Ordering;
use std::convert::TryFrom;

/// A boolean variable.
/// It is a wrapper around an (untyped) discrete variable to provide type safety.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct BVar(VarRef);

impl BVar {
    pub fn new(dvar: VarRef) -> Self {
        BVar(dvar)
    }

    /// Provides an integer view of this boolean variable
    /// where true <-> 1   and  false <-> 0
    pub fn int_view(self) -> IVar {
        IVar::new(self.0)
    }

    pub fn true_lit(self) -> Lit {
        Lit::geq(self, 1)
    }
    pub fn false_lit(self) -> Lit {
        Lit::leq(self, 0)
    }
}

impl From<BVar> for VarRef {
    fn from(i: BVar) -> Self {
        i.0
    }
}

impl From<usize> for BVar {
    fn from(i: usize) -> Self {
        BVar(VarRef::from(i))
    }
}

impl From<BVar> for usize {
    fn from(b: BVar) -> Self {
        usize::from(b.0)
    }
}

impl From<BVar> for IVar {
    fn from(b: BVar) -> Self {
        IVar::new(b.0)
    }
}

impl std::ops::Not for BVar {
    type Output = Lit;

    fn not(self) -> Self::Output {
        self.false_lit()
    }
}

// equivalent to lit
#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub enum BAtom {
    Cst(bool),
    Bound(Lit),
    Expr(BExpr),
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct BExpr {
    pub expr: ExprHandle,
    pub negated: bool,
}

impl std::ops::Not for BExpr {
    type Output = Self;

    fn not(self) -> Self::Output {
        BExpr {
            expr: self.expr,
            negated: !self.negated,
        }
    }
}

impl BAtom {
    pub fn lexical_cmp(&self, other: &BAtom) -> Ordering {
        match self {
            BAtom::Cst(c1) => match other {
                BAtom::Cst(c2) => c1.cmp(c2),
                _ => Ordering::Less,
            },
            BAtom::Bound(x) => match other {
                BAtom::Bound(y) => x.lexical_cmp(y),
                BAtom::Cst(_) => Ordering::Greater,
                BAtom::Expr(_) => Ordering::Less,
            },

            BAtom::Expr(e) => match other {
                BAtom::Expr(e2) => e.cmp(e2),
                BAtom::Cst(_) => Ordering::Greater,
                BAtom::Bound(_) => Ordering::Greater,
            },
        }
    }
}

impl std::ops::Not for BAtom {
    type Output = BAtom;

    fn not(self) -> Self::Output {
        match self {
            BAtom::Cst(x) => BAtom::Cst(!x),
            BAtom::Bound(b) => BAtom::Bound(!b),
            BAtom::Expr(e) => BAtom::Expr(!e),
        }
    }
}

impl From<bool> for BAtom {
    fn from(value: bool) -> Self {
        BAtom::Cst(value)
    }
}

impl From<Lit> for BAtom {
    fn from(bnd: Lit) -> Self {
        BAtom::Bound(bnd)
    }
}

impl From<BVar> for BAtom {
    fn from(var: BVar) -> Self {
        BAtom::Bound(var.into())
    }
}

impl From<BExpr> for BAtom {
    fn from(e: BExpr) -> Self {
        BAtom::Expr(e)
    }
}

impl TryFrom<BAtom> for bool {
    type Error = ConversionError;

    fn try_from(value: BAtom) -> Result<Self, Self::Error> {
        match value {
            BAtom::Cst(b) => Ok(b),
            _ => Err(ConversionError::NotConstant),
        }
    }
}

impl TryFrom<BAtom> for Lit {
    type Error = ConversionError;

    fn try_from(value: BAtom) -> Result<Self, Self::Error> {
        match value {
            BAtom::Bound(b) => Ok(b),
            _ => Err(ConversionError::NotBound),
        }
    }
}

impl TryFrom<BAtom> for BExpr {
    type Error = ConversionError;

    fn try_from(value: BAtom) -> Result<Self, Self::Error> {
        match value {
            BAtom::Expr(b) => Ok(b),
            _ => Err(ConversionError::NotExpression),
        }
    }
}
