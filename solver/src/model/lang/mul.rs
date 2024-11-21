use crate::{
    core::{Lit, VarRef},
    reif::ReifExpr,
};
use std::fmt::{Debug, Formatter};

pub struct EqVarMulLit {
    pub lhs: VarRef,
    pub rhs: VarRef,
    pub lit: Lit,
}

impl Debug for EqVarMulLit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.lit, self.rhs)
    }
}

impl EqVarMulLit {
    pub fn new(lhs: impl Into<VarRef>, rhs: impl Into<VarRef>, lit: impl Into<Lit>) -> Self {
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
    pub lhs: VarRef,
    pub rhs: VarRef,
    pub lit: Lit,
}

impl Debug for NFEqVarMulLit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?} * {:?}", self.lhs, self.lit, self.rhs)
    }
}
