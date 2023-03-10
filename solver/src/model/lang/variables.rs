use crate::core::*;
use crate::model::lang::variables::Variable::*;
use crate::model::lang::{BVar, ConversionError, FVar, IVar, Kind, SVar};
use std::convert::TryFrom;

/// Contains a variable of any type
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub enum Variable {
    Bool(BVar),
    Int(IVar),
    Fixed(FVar),
    Sym(SVar),
}

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

impl From<IVar> for Variable {
    fn from(x: IVar) -> Self {
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

impl From<Variable> for VarRef {
    fn from(v: Variable) -> Self {
        match v {
            Bool(x) => x.into(),
            Int(x) => x.into(),
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

impl TryFrom<Variable> for IVar {
    type Error = ConversionError;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        match value {
            Int(x) => Ok(x),
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
