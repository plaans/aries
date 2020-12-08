use smallvec::alloc::fmt::Formatter;
use smallvec::SmallVec;

use std::convert::TryFrom;
use std::hash::Hash;

pub type IntCst = i32;

use aries_collections::create_ref_type;
create_ref_type!(IVar);
create_ref_type!(BVar);

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
}

impl From<IVar> for IAtom {
    fn from(v: IVar) -> Self {
        IAtom::new(Some(v), 0)
    }
}
impl From<IVar> for Atom {
    fn from(v: IVar) -> Self {
        Atom::from(IAtom::from(v))
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
            Atom::Int(i) => Ok(i),
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

// equivalent to lit
#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct BAtom {
    pub var: Option<BVar>,
    pub negated: bool,
}
impl BAtom {
    pub fn new(var: Option<BVar>, negated: bool) -> BAtom {
        BAtom { var, negated }
    }
}

impl std::ops::Not for BAtom {
    type Output = BAtom;

    fn not(self) -> Self::Output {
        BAtom::new(self.var, !self.negated)
    }
}

impl From<bool> for BAtom {
    fn from(value: bool) -> Self {
        BAtom {
            var: None,
            negated: !value,
        }
    }
}

impl From<BVar> for BAtom {
    fn from(b: BVar) -> Self {
        BAtom::new(Some(b), false)
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

#[derive(Debug)]
pub struct TypeError;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub enum Atom {
    Bool(BAtom),
    Int(IAtom),
}

impl From<BAtom> for Atom {
    fn from(b: BAtom) -> Self {
        Atom::Bool(b)
    }
}

impl From<IAtom> for Atom {
    fn from(i: IAtom) -> Self {
        Atom::Int(i)
    }
}

pub type Args = SmallVec<[Atom; 4]>;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub enum Fun {
    And,
    Or,
    Eq,
    Leq,
}

impl std::fmt::Display for Fun {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Fun::And => "and",
                Fun::Or => "or",
                Fun::Eq => "=",
                Fun::Leq => "<=",
            }
        )
    }
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct Expr {
    pub fun: Fun,
    pub args: Args,
}
impl Expr {
    pub fn new(fun: Fun, args: &[Atom]) -> Expr {
        Expr {
            fun,
            args: Args::from(args),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Model;

    fn check(m: &Model, x: impl Into<Atom>, result: &str) {
        assert_eq!(m.fmt(x).to_string(), result);
    }

    #[test]
    fn test_syntax() {
        let mut m = Model::default();

        let a = m.new_ivar(0, 10, "a");
        check(&m, a, "a");

        let b = m.new_ivar(0, 10, "b");

        let x = b + 1;
        check(&m, x, "(+ b 1)");

        let x = b - 1;
        check(&m, x, "(- b 1)");

        let x = x + 1;
        check(&m, x, "b");

        let x = m.leq(a + 1, 6);
        check(&m, x, "(<= (+ a 1) 6)");

        let x = m.eq(a - 3, b);
        check(&m, x, "(= (- a 3) b)");

        let x = m.implies(true, x);
        check(&m, x, "(or false (= (- a 3) b))")
    }
}
