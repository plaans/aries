use crate::core::IntCst;
use crate::model::lang::{Atom, ConversionError};
use crate::model::symbols::TypedSym;

use super::fixed::Rational;

/// Represents a constant value
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub enum Cst {
    Int(IntCst),
    Fixed(Rational),
    Sym(TypedSym),
    Bool(bool),
}

impl From<Cst> for Atom {
    fn from(value: Cst) -> Self {
        match value {
            Cst::Int(i) => i.into(),
            Cst::Fixed(f) => f.into(),
            Cst::Sym(s) => s.into(),
            Cst::Bool(b) => b.into(),
        }
    }
}

impl TryFrom<Atom> for Cst {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Sym(c) => Ok(Cst::Sym(c.try_into()?)),
            Atom::Int(i) => Ok(Cst::Int(i.try_into()?)),
            Atom::Fixed(f) => Ok(Cst::Fixed(f.try_into()?)),
            Atom::Bool(l) => Ok(Cst::Bool(l.try_into()?)),
        }
    }
}

impl From<IntCst> for Cst {
    fn from(value: IntCst) -> Self {
        Cst::Int(value)
    }
}

impl From<Rational> for Cst {
    fn from(value: Rational) -> Self {
        Cst::Fixed(value)
    }
}

impl From<bool> for Cst {
    fn from(value: bool) -> Self {
        Cst::Bool(value)
    }
}

impl From<TypedSym> for Cst {
    fn from(value: TypedSym) -> Self {
        Cst::Sym(value)
    }
}
