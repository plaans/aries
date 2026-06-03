use crate::legacy::*;
use aries::{model::lang::BVar, prelude::*};
use std::convert::TryFrom;

/// Contains a variable of any type
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub enum Variable {
    Bool(BVar),
    Int(Var),
    Fixed(FVar),
    Sym(SVar),
}

use Variable::*;

impl Variable {
    pub fn kind(self) -> Kind {
        match self {
            Bool(_) => Kind::Bool,
            Int(_) => Kind::Int,
            Fixed(f) => Kind::Fixed(f.denom),
            Sym(_) => Kind::Sym,
        }
    }
}

impl From<BVar> for Variable {
    fn from(x: BVar) -> Self {
        Bool(x)
    }
}

impl From<Var> for Variable {
    fn from(x: Var) -> Self {
        Int(x)
    }
}

impl From<SVar> for Variable {
    fn from(x: SVar) -> Self {
        Sym(x)
    }
}

impl From<FVar> for Variable {
    fn from(f: FVar) -> Self {
        Fixed(f)
    }
}

impl From<Variable> for Var {
    fn from(v: Variable) -> Self {
        match v {
            Bool(x) => x.into(),
            Int(x) => x,
            Fixed(x) => x.into(),
            Sym(x) => x.into(),
        }
    }
}

impl TryFrom<Variable> for BVar {
    type Error = ConversionError;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Bool(x) => Ok(x),
            _ => Err(ConversionError::TypeError),
        }
    }
}

impl TryFrom<Variable> for SVar {
    type Error = ConversionError;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Sym(x) => Ok(x),
            _ => Err(ConversionError::TypeError),
        }
    }
}
