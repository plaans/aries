use smallvec::SmallVec;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::hash::Hash;

type IntCst = i32;

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct IVar(u32);
#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct BVar(u32);

// var + cst
#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct IAtom {
    var: Option<IVar>,
    shift: IntCst,
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
impl From<IntCst> for IAtom {
    fn from(i: i32) -> Self {
        IAtom::new(None, i)
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

// equivalent to lit
#[derive(Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct BAtom {
    var: Option<BVar>,
    negated: bool,
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

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Fun {
    And,
    Or,
    Eq,
    Leq,
}

#[derive(Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Expr {
    fun: Fun,
    args: Args,
}
impl Expr {
    pub fn new(fun: Fun, args: &[Atom]) -> Expr {
        Expr {
            fun,
            args: Args::from(args),
        }
    }
}

type Label = String;
struct IntVarDesc {
    lb: IntCst,
    ub: IntCst,
    label: Option<Label>,
}
impl IntVarDesc {
    pub fn new(lb: IntCst, ub: IntCst, label: Option<Label>) -> IntVarDesc {
        IntVarDesc { lb, ub, label }
    }
}

#[derive(Default)]
pub struct Interner {
    bools: Vec<Option<Label>>,
    ints: Vec<IntVarDesc>,
    interned: HashMap<Expr, Atom>,
}

impl Interner {
    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        let id = BVar(self.bools.len() as u32);
        let label = label.into();
        let label = if label.len() == 0 { None } else { Some(label) };
        self.bools.push(label);
        id
    }

    pub fn new_ivar<L: Into<Label>>(&mut self, lb: IntCst, ub: IntCst, label: L) -> IVar {
        let id = IVar(self.ints.len() as u32);
        let label = label.into();
        let label = if label.len() == 0 { None } else { Some(label) };
        self.ints.push(IntVarDesc::new(lb, ub, label));
        id
    }

    pub fn intern_bool(&mut self, e: Expr) -> Result<BAtom, TypeError> {
        if self.interned.contains_key(&e) {
            let atom = self.interned[&e];
            atom.try_into()
        } else {
            let key = BAtom::from(self.new_bvar(""));
            self.interned.insert(e, key.into());
            Ok(key)
        }
    }

    pub fn and2(&mut self, a: BAtom, b: BAtom) -> BAtom {
        let and = Expr::new(Fun::And, &[a.into(), b.into()]);
        self.intern_bool(and).expect("")
    }
    pub fn or2(&mut self, a: BAtom, b: BAtom) -> BAtom {
        let and = Expr::new(Fun::Or, &[a.into(), b.into()]);
        self.intern_bool(and).expect("")
    }

    pub fn leq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        let leq = Expr::new(Fun::Leq, &[a.into(), b.into()]);
        self.intern_bool(leq).expect("")
    }

    pub fn eq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        let eq = Expr::new(Fun::Eq, &[a.into(), b.into()]);
        self.intern_bool(eq).expect("")
    }

    pub fn neq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        !self.eq(a, b)
    }

    pub fn implies<A: Into<BAtom>, B: Into<BAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        let implication = Expr::new(Fun::Or, &[Atom::from(!a), Atom::from(b)]);
        self.intern_bool(implication).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax() {
        let mut i = Interner::default();

        let a = i.new_ivar(0, 10, "a");
        let b = i.new_ivar(0, 10, "b");
        let _ = i.leq(a + 1, 6);
        let x = i.eq(a + 3, b);
        let _ = i.implies(true, x);
    }
}
