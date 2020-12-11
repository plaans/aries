use super::*;
use std::convert::TryInto;

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

impl From<IAtom> for Atom {
    fn from(i: IAtom) -> Self {
        Atom::Disc(i.into())
    }
}

impl From<SAtom> for Atom {
    fn from(s: SAtom) -> Self {
        Atom::Disc(s.into())
    }
}

impl From<DAtom> for Atom {
    fn from(d: DAtom) -> Self {
        Atom::Disc(d)
    }
}

impl From<BVar> for Atom {
    fn from(v: BVar) -> Self {
        Atom::from(BAtom::from(v))
    }
}

impl From<IVar> for Atom {
    fn from(v: IVar) -> Self {
        Atom::from(IAtom::from(v))
    }
}

impl From<SVar> for Atom {
    fn from(v: SVar) -> Self {
        Atom::from(SAtom::from(v))
    }
}

impl From<bool> for Atom {
    fn from(b: bool) -> Self {
        BAtom::from(b).into()
    }
}

impl From<IntCst> for Atom {
    fn from(i: IntCst) -> Self {
        IAtom::from(i).into()
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

impl TryFrom<Atom> for SAtom {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Bool(_) => Err(ConversionError::TypeError),
            Atom::Disc(d) => d.try_into(),
        }
    }
}

impl TryFrom<Atom> for IAtom {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Bool(_) => Err(ConversionError::TypeError),
            Atom::Disc(d) => d.try_into(),
        }
    }
}

impl TryFrom<Atom> for bool {
    type Error = ConversionError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        let batom = BAtom::try_from(value)?;
        batom.try_into()
    }
}
