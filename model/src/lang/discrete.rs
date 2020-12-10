use crate::lang::sym::{SAtom, VarOrSym};
use crate::lang::{IAtom, IVar, IntCst, TypeError};
use crate::symbols::SymId;
use crate::types::TypeId;
use serde::export::TryFrom;

/// A discrete atom, representing either a symbol or an integer.
#[derive(Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct DAtom {
    var: Option<IVar>,
    shift: IntCst,
    /// Type of the Atom:
    ///  - Some(tpe): this a symbol with type tpe
    ///  - None: this is an integer
    tpe: Option<TypeId>,
}

impl From<IAtom> for DAtom {
    fn from(i: IAtom) -> Self {
        DAtom {
            var: i.var,
            shift: i.shift,
            tpe: None,
        }
    }
}

impl TryFrom<DAtom> for IAtom {
    type Error = TypeError;

    fn try_from(value: DAtom) -> Result<Self, Self::Error> {
        match value.tpe {
            Some(_) => Err(TypeError),
            None => Ok(IAtom::new(value.var, value.shift)),
        }
    }
}

impl From<SAtom> for DAtom {
    fn from(s: SAtom) -> Self {
        match s.atom {
            VarOrSym::Var(v) => DAtom {
                var: Some(v),
                shift: 0,
                tpe: Some(s.tpe),
            },
            VarOrSym::Sym(sym) => DAtom {
                var: None,
                shift: usize::from(sym) as IntCst,
                tpe: Some(s.tpe),
            },
        }
    }
}

impl TryFrom<DAtom> for SAtom {
    type Error = TypeError;

    fn try_from(value: DAtom) -> Result<Self, Self::Error> {
        match value.tpe {
            Some(tpe) => match value.var {
                None => Ok(SAtom::new_constant(SymId::from(value.shift as usize), tpe)),
                Some(var) => {
                    assert_eq!(value.shift, 0);
                    Ok(SAtom::new_variable(var, tpe))
                }
            },
            None => Err(TypeError),
        }
    }
}
