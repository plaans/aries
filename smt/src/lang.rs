use smallvec::alloc::fmt::Formatter;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::hash::Hash;

pub type IntCst = i32;

use crate::model::{BoolModel, IntDomain, IntModel};
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
    Neq,
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
                Fun::Neq => "!=",
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
pub struct Model {
    pub bools: BoolModel,
    pub ints: IntModel,
    pub expressions: Expressions,
}

#[derive(Default)]
pub struct Expressions {
    interned: HashMap<Expr, Atom>,
    expressions: HashMap<Atom, Expr>,
}
impl Expressions {
    pub fn contains_expr(&self, expr: &Expr) -> bool {
        self.interned.contains_key(expr)
    }

    pub fn atom_of(&self, expr: &Expr) -> Option<Atom> {
        self.interned.get(expr).copied()
    }

    pub fn expr_of(&self, atom: impl Into<Atom>) -> Option<&Expr> {
        self.expressions.get(&atom.into())
    }

    pub fn bind(&mut self, atom: Atom, expr: Expr) {
        self.interned.insert(expr.clone(), atom);
        self.expressions.insert(atom, expr);
    }
}

impl Model {
    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        self.bools.new_bvar(label)
    }

    pub fn new_ivar<L: Into<Label>>(&mut self, lb: IntCst, ub: IntCst, label: L) -> IVar {
        self.ints.new_ivar(lb, ub, label)
    }

    /// Wraps an atom into a custom object that can be formatted with the standard library `Display`
    ///
    /// Expressions and variables are formatted into a single line with lisp-like syntax.
    /// Anonymous variables are prefixed with "b_" and "i_" (for bools and ints respectively followed
    /// by a unique identifier.
    ///
    /// # Usage
    /// ```
    /// use aries_smt::lang::Model;
    /// let mut i = Model::default();
    /// let x = i.new_ivar(0, 10, "X");
    /// let y = x + 10;
    /// println!("x: {}", i.fmt(x));
    /// println!("y: {}", i.fmt(y));
    /// ```
    pub fn fmt(&self, atom: impl Into<Atom>) -> impl std::fmt::Display + '_ {
        // a custom type to extract the formatter and feed it to formal_impl
        // source: https://github.com/rust-lang/rust/issues/46591#issuecomment-350437057
        struct Fmt<F>(pub F)
        where
            F: Fn(&mut std::fmt::Formatter) -> std::fmt::Result;

        impl<F> std::fmt::Display for Fmt<F>
        where
            F: Fn(&mut std::fmt::Formatter) -> std::fmt::Result,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                (self.0)(f)
            }
        }
        let atom = atom.into();
        Fmt(move |f| self.format_impl(atom, f))
    }

    fn format_impl(&self, atom: Atom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.expr_of(atom) {
            Some(e) => {
                write!(f, "({}", e.fun)?;
                for arg in &e.args {
                    write!(f, " ")?;
                    self.format_impl(*arg, f)?;
                }
                write!(f, ")")
            }
            None => match atom {
                Atom::Bool(b) => match b.var {
                    None => write!(f, "{}", !b.negated),
                    Some(v) => {
                        if b.negated {
                            write!(f, "!")?
                        }
                        if let Some(lbl) = &self.bools.label(v) {
                            write!(f, "{}", lbl)
                        } else {
                            write!(f, "b_{}", usize::from(v))
                        }
                    }
                },
                Atom::Int(i) => match i.var {
                    None => write!(f, "{}", i.shift),
                    Some(v) => {
                        if i.shift > 0 {
                            write!(f, "(+ ")?;
                        } else if i.shift < 0 {
                            write!(f, "(- ")?;
                        }
                        if let Some(lbl) = self.ints.label(v) {
                            write!(f, "{}", lbl)?;
                        } else {
                            write!(f, "i_{}", usize::from(v))?;
                        }
                        if i.shift != 0 {
                            write!(f, " {})", i.shift.abs())?;
                        }
                        std::fmt::Result::Ok(())
                    }
                },
            },
        }
    }

    pub fn bounds(&self, ivar: IVar) -> (IntCst, IntCst) {
        let IntDomain { lb, ub, .. } = self.ints.domain_of(ivar);
        (*lb, *ub)
    }

    pub fn expr_of(&self, atom: impl Into<Atom>) -> Option<&Expr> {
        self.expressions.expr_of(atom)
    }

    pub fn intern_bool(&mut self, e: Expr) -> Result<BAtom, TypeError> {
        match self.expressions.atom_of(&e) {
            Some(atom) => atom.try_into(),
            None => {
                let key = BAtom::from(self.new_bvar(""));
                self.expressions.bind(key.into(), e);
                Ok(key)
            }
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

    pub fn lt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        self.leq(a + 1, b)
    }

    pub fn eq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        let eq = Expr::new(Fun::Eq, &[a.into(), b.into()]);
        self.intern_bool(eq).expect("")
    }

    pub fn neq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        let eq = Expr::new(Fun::Neq, &[a.into(), b.into()]);
        self.intern_bool(eq).expect("")
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

    fn check(i: &Model, x: impl Into<Atom>, result: &str) {
        assert_eq!(i.fmt(x).to_string(), result);
    }

    #[test]
    fn test_syntax() {
        let mut i = Model::default();

        let a = i.new_ivar(0, 10, "a");
        check(&i, a, "a");

        let b = i.new_ivar(0, 10, "b");

        let x = b + 1;
        check(&i, x, "(+ b 1)");

        let x = b - 1;
        check(&i, x, "(- b 1)");

        let x = x + 1;
        check(&i, x, "b");

        let x = i.leq(a + 1, 6);
        check(&i, x, "(<= (+ a 1) 6)");

        let x = i.eq(a - 3, b);
        check(&i, x, "(= (- a 3) b)");

        let x = i.implies(true, x);
        check(&i, x, "(or false (= (- a 3) b))")
    }
}
