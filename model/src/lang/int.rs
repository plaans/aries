use crate::lang::{Atom, DVar, IntCst, TypeError};
use std::cmp::Ordering;
use std::convert::{TryFrom, TryInto};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct IVar(DVar);

impl IVar {
    pub fn new(dvar: DVar) -> Self {
        IVar(dvar)
    }
}

impl From<IVar> for DVar {
    fn from(i: IVar) -> Self {
        i.0
    }
}

// var + cst
#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct IAtom {
    pub var: Option<IVar>,
    pub shift: IntCst,
}
impl IAtom {
    pub fn new(var: Option<IVar>, shift: IntCst) -> IAtom {
        IAtom { var, shift }
    }

    pub fn lexical_cmp(&self, other: &IAtom) -> Ordering {
        match (self.var, other.var) {
            (Some(v1), Some(v2)) if v1 != v2 => v1.cmp(&v2),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            _ => self.shift.cmp(&other.shift),
        }
    }
}

impl From<IVar> for IAtom {
    fn from(v: IVar) -> Self {
        IAtom::new(Some(v), 0)
    }
}
impl From<IntCst> for IAtom {
    fn from(i: i32) -> Self {
        IAtom::new(None, i)
    }
}
impl TryFrom<Atom> for IAtom {
    type Error = TypeError;

    fn try_from(atom: Atom) -> Result<Self, Self::Error> {
        match atom {
            Atom::Disc(i) => i.try_into(),
            _ => Err(TypeError),
        }
    }
}

impl std::ops::Add<IntCst> for IAtom {
    type Output = IAtom;

    fn add(self, rhs: IntCst) -> Self::Output {
        IAtom::new(self.var, self.shift + rhs)
    }
}
impl std::ops::Add<IntCst> for IVar {
    type Output = IAtom;

    fn add(self, rhs: IntCst) -> Self::Output {
        IAtom::new(Some(self), rhs)
    }
}
impl std::ops::Sub<IntCst> for IAtom {
    type Output = IAtom;

    fn sub(self, rhs: IntCst) -> Self::Output {
        IAtom::new(self.var, self.shift - rhs)
    }
}
impl std::ops::Sub<IntCst> for IVar {
    type Output = IAtom;

    fn sub(self, rhs: IntCst) -> Self::Output {
        IAtom::new(Some(self), -rhs)
    }
}
