use crate::lang::{ConversionError, IAtom, IVar, IntCst, VarRef};
use crate::symbols::{SymId, TypedSym};
use crate::types::TypeId;
use std::convert::TryFrom;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct SVar {
    pub var: VarRef,
    pub tpe: TypeId,
}

impl SVar {
    pub fn new(var: VarRef, tpe: TypeId) -> Self {
        SVar { var, tpe }
    }
}

/// Atom representing a symbol, either a constant one or a variable.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub enum SAtom {
    Var(SVar),
    Cst(TypedSym),
}

impl SAtom {
    pub fn new_constant(sym: SymId, tpe: TypeId) -> Self {
        SAtom::Cst(TypedSym { sym, tpe })
    }

    pub fn new_variable(svar: SVar) -> Self {
        SAtom::Var(svar)
    }

    /// Returns the type of this atom
    pub fn tpe(&self) -> TypeId {
        match self {
            SAtom::Var(v) => v.tpe,
            SAtom::Cst(c) => c.tpe,
        }
    }

    pub fn int_view(self) -> IAtom {
        match self {
            SAtom::Var(v) => IAtom::new(IVar::new(v.var), 0),
            SAtom::Cst(s) => IAtom::new(IVar::ZERO, usize::from(s.sym) as IntCst),
        }
    }
}

impl From<SVar> for VarRef {
    fn from(s: SVar) -> Self {
        s.var
    }
}

impl From<SVar> for SAtom {
    fn from(v: SVar) -> Self {
        SAtom::new_variable(v)
    }
}

impl From<TypedSym> for SAtom {
    fn from(s: TypedSym) -> Self {
        SAtom::new_constant(s.sym, s.tpe)
    }
}

impl TryFrom<SAtom> for SVar {
    type Error = ConversionError;

    fn try_from(value: SAtom) -> Result<Self, Self::Error> {
        match value {
            SAtom::Var(v) => Ok(v),
            SAtom::Cst(_) => Err(ConversionError::NotVariable),
        }
    }
}

impl TryFrom<SAtom> for SymId {
    type Error = ConversionError;

    fn try_from(value: SAtom) -> Result<Self, Self::Error> {
        TypedSym::try_from(value).map(SymId::from)
    }
}

impl TryFrom<SAtom> for TypedSym {
    type Error = ConversionError;

    fn try_from(value: SAtom) -> Result<Self, Self::Error> {
        match value {
            SAtom::Var(_) => Err(ConversionError::NotConstant),
            SAtom::Cst(s) => Ok(s),
        }
    }
}
