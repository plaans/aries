use crate::lang::sym::{SAtom, SVar, VarOrSym};
use crate::lang::{DVar, IAtom, IVar, IntCst, TypeError};
use crate::symbols::SymId;
use crate::types::TypeId;
use serde::export::TryFrom;

#[derive(Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct DiscreteType {
    inner: Option<TypeId>,
}
impl DiscreteType {
    pub fn new_symbolic(tpe: TypeId) -> Self {
        DiscreteType { inner: Some(tpe) }
    }
    pub fn integer() -> Self {
        DiscreteType { inner: None }
    }

    pub fn to_symbolic(self) -> Option<TypeId> {
        self.inner
    }

    pub fn is_integer(self) -> bool {
        self.inner.is_none()
    }
}

/// A discrete atom, representing either a symbol or an integer.
#[derive(Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct DAtom {
    var: Option<DVar>,
    shift: IntCst,
    /// Type of the Atom:
    ///  - Some(tpe): this a symbol with type tpe
    ///  - None: this is an integer
    tpe: DiscreteType,
}

impl From<IAtom> for DAtom {
    fn from(i: IAtom) -> Self {
        DAtom {
            var: i.var.map(DVar::from),
            shift: i.shift,
            tpe: DiscreteType::integer(),
        }
    }
}

impl From<IVar> for DAtom {
    fn from(i: IVar) -> Self {
        IAtom::from(i).into()
    }
}

impl From<IntCst> for DAtom {
    fn from(i: IntCst) -> Self {
        IAtom::from(i).into()
    }
}

impl From<SVar> for DAtom {
    fn from(s: SVar) -> Self {
        SAtom::from(s).into()
    }
}

impl TryFrom<DAtom> for IAtom {
    type Error = TypeError;

    fn try_from(value: DAtom) -> Result<Self, Self::Error> {
        if value.tpe.is_integer() {
            Ok(IAtom::new(value.var.map(IVar::new), value.shift))
        } else {
            Err(TypeError)
        }
    }
}

impl From<SAtom> for DAtom {
    fn from(s: SAtom) -> Self {
        match s.atom {
            VarOrSym::Var(v) => DAtom {
                var: Some(v),
                shift: 0,
                tpe: DiscreteType::new_symbolic(s.tpe),
            },
            VarOrSym::Sym(sym) => DAtom {
                var: None,
                shift: usize::from(sym) as IntCst,
                tpe: DiscreteType::new_symbolic(s.tpe),
            },
        }
    }
}

impl TryFrom<DAtom> for SAtom {
    type Error = TypeError;

    fn try_from(value: DAtom) -> Result<Self, Self::Error> {
        match value.tpe.to_symbolic() {
            Some(tpe) => match value.var {
                None => Ok(SAtom::new_constant(SymId::from(value.shift as usize), tpe)),
                Some(var) => {
                    assert_eq!(value.shift, 0);
                    let svar = SVar::new(var, tpe);
                    Ok(svar.into())
                }
            },
            None => Err(TypeError),
        }
    }
}
