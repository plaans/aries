use super::*;
use crate::symbols::TypedSym;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub enum Atom {
    Bool(BAtom),
    Int(IAtom),
    Sym(SAtom),
}

impl From<BAtom> for Atom {
    fn from(b: BAtom) -> Self {
        Atom::Bool(b)
    }
}

impl From<IAtom> for Atom {
    fn from(d: IAtom) -> Self {
        Atom::Int(d)
    }
}

impl From<SAtom> for Atom {
    fn from(s: SAtom) -> Self {
        Atom::Sym(s)
    }
}

impl From<Variable> for Atom {
    fn from(v: Variable) -> Self {
        match v {
            Variable::Bool(b) => Self::Bool(b.into()),
            Variable::Int(i) => Self::Int(i.into()),
            Variable::Sym(s) => Self::Sym(s.into()),
        }
    }
}

impl TryFrom<Atom> for BAtom {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Bool(b) => Ok(b),
            _ => Err(ConversionError::TypeError),
        }
    }
}
impl TryFrom<Atom> for IAtom {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Int(i) => Ok(i),
            _ => Err(ConversionError::TypeError),
        }
    }
}

impl TryFrom<Atom> for SAtom {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Sym(s) => Ok(s),
            _ => Err(ConversionError::TypeError),
        }
    }
}

impl TryFrom<Atom> for Variable {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        Ok(match value {
            Atom::Bool(x) => Variable::Bool(x.try_into()?),
            Atom::Int(i) => Variable::Int(i.try_into()?),
            Atom::Sym(s) => Variable::Sym(s.try_into()?),
        })
    }
}

use crate::transitive_conversions;
use std::convert::TryInto;
transitive_conversions!(Atom, BAtom, BVar);
transitive_conversions!(Atom, BAtom, bool);
transitive_conversions!(Atom, IAtom, IVar);
transitive_conversions!(Atom, IAtom, IntCst);
transitive_conversions!(Atom, SAtom, SVar);
transitive_conversions!(Atom, SAtom, TypedSym);
