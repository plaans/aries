use crate::lang::{IAtom, IVar, IntCst, TypeError};
use crate::symbols::SymId;
use crate::types::TypeId;
use std::convert::TryFrom;

/// Atom representing a symbol, either a constant one or a variable.
pub struct SAtom {
    pub(crate) atom: VarOrSym,
    pub(crate) tpe: TypeId,
}
pub(crate) enum VarOrSym {
    Var(IVar),
    Sym(SymId),
}

impl SAtom {
    pub fn new_constant(sym: SymId, tpe: TypeId) -> Self {
        SAtom {
            atom: VarOrSym::Sym(sym),
            tpe,
        }
    }

    pub fn new_variable(ivar: IVar, tpe: TypeId) -> Self {
        SAtom {
            atom: VarOrSym::Var(ivar),
            tpe,
        }
    }

    pub fn to_int(self) -> IAtom {
        match self.atom {
            VarOrSym::Var(v) => IAtom::new(Some(v), 0),
            VarOrSym::Sym(s) => IAtom::new(None, usize::from(s) as IntCst),
        }
    }
}

impl TryFrom<SAtom> for SymId {
    type Error = TypeError;

    fn try_from(value: SAtom) -> Result<Self, Self::Error> {
        match value.atom {
            VarOrSym::Var(_) => Err(TypeError),
            VarOrSym::Sym(s) => Ok(s),
        }
    }
}

impl TryFrom<SAtom> for IVar {
    type Error = TypeError;

    fn try_from(value: SAtom) -> Result<Self, Self::Error> {
        match value.atom {
            VarOrSym::Var(v) => Ok(v),
            VarOrSym::Sym(_) => Err(TypeError),
        }
    }
}
