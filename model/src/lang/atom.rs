use super::*;

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

impl TryFrom<Atom> for BAtom {
    type Error = TypeError;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Bool(b) => Ok(b),
            _ => Err(TypeError),
        }
    }
}

// TODO: reomve, an ivar can be either a symbol or an integer
impl From<IVar> for Atom {
    fn from(v: IVar) -> Self {
        Atom::from(IAtom::from(v))
    }
}
