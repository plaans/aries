use crate::core::IntCst;
use crate::model::lang::{Atom, ConversionError};
use crate::model::symbols::TypedSym;
use num_rational::Rational32;

/// Represents a constant value
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub enum Cst {
    Int(IntCst),
    Fixed(Rational32),
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
