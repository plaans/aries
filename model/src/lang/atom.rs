use super::*;
use crate::symbols::TypedSym;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub enum Atom {
    Bool(BAtom),
    Disc(DAtom),
}

impl From<BAtom> for Atom {
    fn from(b: BAtom) -> Self {
        Atom::Bool(b)
    }
}

impl From<DAtom> for Atom {
    fn from(d: DAtom) -> Self {
        Atom::Disc(d)
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
impl TryFrom<Atom> for DAtom {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Bool(_) => Err(ConversionError::TypeError),
            Atom::Disc(d) => Ok(d),
        }
    }
}

use crate::transitive_conversions;
transitive_conversions!(Atom, DAtom, IAtom);
transitive_conversions!(Atom, DAtom, SAtom);
transitive_conversions!(Atom, BAtom, BVar);
transitive_conversions!(Atom, BAtom, bool);
transitive_conversions!(Atom, IAtom, IVar);
transitive_conversions!(Atom, IAtom, IntCst);
transitive_conversions!(Atom, SAtom, SVar);
transitive_conversions!(Atom, SAtom, TypedSym);
