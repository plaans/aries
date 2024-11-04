use crate::core::{Lit, VarRef};
use std::fmt::{Debug, Formatter};

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
