use crate::lang::{ConversionError, DVar, IAtom, IVar, IntCst};
use crate::symbols::{SymId, TypedSym};
use crate::types::TypeId;
use std::convert::TryFrom;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct SVar(DVar, TypeId);

impl SVar {
    pub fn new(var: DVar, tpe: TypeId) -> Self {
        SVar(var, tpe)
    }
}

/// Atom representing a symbol, either a constant one or a variable.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct SAtom {
    pub atom: VarOrSym,
    pub tpe: TypeId,
}
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub enum VarOrSym {
    Var(DVar),
    Sym(SymId),
}

impl SAtom {
    pub fn new_constant(sym: SymId, tpe: TypeId) -> Self {
        SAtom {
            atom: VarOrSym::Sym(sym),
            tpe,
        }
    }

    pub fn new_variable(svar: SVar) -> Self {
        SAtom {
            atom: VarOrSym::Var(svar.0),
            tpe: svar.1,
        }
    }

    pub fn to_int(self) -> IAtom {
        match self.atom {
            VarOrSym::Var(v) => IAtom::new(Some(IVar::new(v)), 0),
            VarOrSym::Sym(s) => IAtom::new(None, usize::from(s) as IntCst),
        }
    }
}

pub struct NotVariable;
pub struct NotConstant;

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
    type Error = NotVariable;

    fn try_from(value: SAtom) -> Result<Self, Self::Error> {
        match value.atom {
            VarOrSym::Var(v) => Ok(SVar(v, value.tpe)),
            VarOrSym::Sym(_) => Err(NotVariable),
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
        match value.atom {
            VarOrSym::Var(_) => Err(ConversionError::NotConstant),
            VarOrSym::Sym(sym) => Ok(TypedSym { sym, tpe: value.tpe }),
        }
    }
}
