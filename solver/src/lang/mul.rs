use crate::{
    core::{Lit, Var},
    reif::ReifExpr,
};
use std::fmt::{Debug, Formatter};

/// Represents the constraint  `lhs = rhs1 * rhs2`
#[derive(Eq, PartialEq, Hash, Clone)]
pub struct EqMul {
    pub lhs: Var,
    pub rhs1: Var,
    pub rhs2: Var,
}

impl EqMul {
    pub fn new(lhs: impl Into<Var>, factor1: impl Into<Var>, factor2: impl Into<Var>) -> Self {
        let factor1 = factor1.into();
        let factor2 = factor2.into();
        let (rhs1, rhs2) = if factor1 <= factor2 {
            (factor1, factor2)
        } else {
            (factor2, factor1)
        };
        Self {
            lhs: lhs.into(),
            rhs1,
            rhs2,
        }
    }
}

impl From<EqMul> for ReifExpr {
    fn from(value: EqMul) -> Self {
        ReifExpr::EqMul(value)
    }
}

impl Debug for EqMul {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.rhs1, self.rhs2)
    }
}

pub struct EqVarMulLit {
    pub lhs: Var,
    pub rhs: Var,
    pub lit: Lit,
}

impl Debug for EqVarMulLit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.lit, self.rhs)
    }
}

impl EqVarMulLit {
    pub fn new(lhs: impl Into<Var>, rhs: impl Into<Var>, lit: impl Into<Lit>) -> Self {
        let lhs = lhs.into();
        let rhs = rhs.into();
        let lit = lit.into();
        Self { lhs, rhs, lit }
    }
}

impl From<EqVarMulLit> for ReifExpr {
    fn from(value: EqVarMulLit) -> Self {
        ReifExpr::EqVarMulLit(NFEqVarMulLit {
            lhs: value.lhs,
            rhs: value.rhs,
            lit: value.lit,
        })
    }
}

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct NFEqVarMulLit {
    pub lhs: Var,
    pub rhs: Var,
    pub lit: Lit,
}

impl Debug for NFEqVarMulLit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.lit, self.rhs)
    }
}
