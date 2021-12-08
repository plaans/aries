use crate::bounds::{Disjunction, Lit};
use crate::lang::normal_form::{NFEq, NFLeq, NFOptEq, NFOptLeq, NormalExpr};
use crate::lang::reification::ReifiableExpr;
use crate::lang::{Atom, FAtom, IAtom};

/// Trait denoting the capability of transforming an expression into its normal form.
///
/// This transformation is necessary to transform user defined constraints into constraints
/// that can be efficiently reified.
///
/// # Example transformations
///
/// ```text
/// - (a = b)   =>  (a = b)
/// - (a != b)  =>  not(a = b)
/// - (b = a)   =>  (a = b)     # sorted by lexical order
/// ```
/// After normalization, a single literal is necessary to reify the 3 expressions.
pub trait Normalize<X: ReifiableExpr> {
    fn normalize(&self) -> NormalExpr<X>;
}
impl Normalize<Never> for Lit {
    fn normalize(&self) -> NormalExpr<Never> {
        NormalExpr::Literal(*self)
    }
}

/// A type that can never be created.
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct Never(());

pub fn leq(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> Leq {
    Leq(lhs.into(), rhs.into())
}

pub fn lt(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> Leq {
    leq(lhs.into(), rhs.into() - 1)
}

pub fn geq(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> Leq {
    leq(rhs, lhs)
}
pub fn gt(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> Leq {
    lt(rhs, lhs)
}

pub fn opt_leq(lhs: impl Into<IAtom>, rhs: impl Into<IAtom>) -> OptLeq {
    OptLeq(lhs.into(), rhs.into())
}

pub fn f_leq(lhs: impl Into<FAtom>, rhs: impl Into<FAtom>) -> Leq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.denom, rhs.denom);
    leq(lhs.num, rhs.num)
}
pub fn f_lt(lhs: impl Into<FAtom>, rhs: impl Into<FAtom>) -> Leq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.denom, rhs.denom);
    lt(lhs.num, rhs.num)
}
pub fn f_opt_leq(lhs: impl Into<FAtom>, rhs: impl Into<FAtom>) -> OptLeq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.denom, rhs.denom);
    opt_leq(lhs.num, rhs.num)
}

pub fn eq(lhs: impl Into<Atom>, rhs: impl Into<Atom>) -> Eq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.kind(), rhs.kind());
    Eq(lhs, rhs)
}

pub fn neq(lhs: impl Into<Atom>, rhs: impl Into<Atom>) -> Neq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.kind(), rhs.kind());
    Neq(lhs, rhs)
}

pub fn opt_eq(lhs: impl Into<Atom>, rhs: impl Into<Atom>) -> OptEq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.kind(), rhs.kind());
    OptEq(lhs, rhs)
}

pub fn or(disjuncts: impl Into<Box<[Lit]>>) -> Or {
    Or(disjuncts.into())
}
pub fn and(disjuncts: impl Into<Box<[Lit]>>) -> And {
    And(disjuncts.into())
}
pub fn implies(a: impl Into<Lit>, b: impl Into<Lit>) -> Or {
    or([!a.into(), b.into()])
}

pub struct Or(Box<[Lit]>);

impl Normalize<Disjunction> for Or {
    fn normalize(&self) -> NormalExpr<Disjunction> {
        let vec = self.0.iter().copied().collect();
        if let Some(disj) = Disjunction::new_non_tautological(vec) {
            NormalExpr::Pos(disj)
        } else {
            NormalExpr::Literal(Lit::TRUE)
        }
    }
}

pub struct And(Box<[Lit]>);

impl Normalize<Disjunction> for And {
    fn normalize(&self) -> NormalExpr<Disjunction> {
        // (and a b c)  <=>  !(or !a !b !c)
        let vec = self.0.iter().copied().map(|l| !l).collect();
        if let Some(disj) = Disjunction::new_non_tautological(vec) {
            NormalExpr::Neg(disj)
        } else {
            NormalExpr::Literal(Lit::FALSE)
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Leq(IAtom, IAtom);

impl std::ops::Not for Leq {
    type Output = Leq;

    fn not(self) -> Self::Output {
        gt(self.0, self.1)
    }
}

impl Normalize<NFLeq> for Leq {
    fn normalize(&self) -> NormalExpr<NFLeq> {
        NFLeq::leq(self.0, self.1)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Eq(Atom, Atom);

impl Normalize<NFEq> for Eq {
    fn normalize(&self) -> NormalExpr<NFEq> {
        assert_eq!(self.0.kind(), self.1.kind());
        NFEq::eq(self.0, self.1)
    }
}

pub struct Neq(Atom, Atom);

impl Normalize<NFEq> for Neq {
    fn normalize(&self) -> NormalExpr<NFEq> {
        assert_eq!(self.0.kind(), self.1.kind());
        !NFEq::eq(self.0, self.1)
    }
}

pub struct OptLeq(IAtom, IAtom);

impl Normalize<NFOptLeq> for OptLeq {
    fn normalize(&self) -> NormalExpr<NFOptLeq> {
        let lhs = self.0.var.into();
        let rhs = self.1.var.into();
        let rhs_add = self.1.shift - self.0.shift;
        NormalExpr::Pos(NFOptLeq { lhs, rhs, rhs_add })
    }
}

pub struct OptEq(Atom, Atom);

impl Normalize<NFOptEq> for OptEq {
    fn normalize(&self) -> NormalExpr<NFOptEq> {
        assert_eq!(self.0.kind(), self.1.kind());
        let a = self.0.int_view().unwrap();
        let b = self.1.int_view().unwrap();
        let lhs = a.var.into();
        let rhs = b.var.into();
        let rhs_add = b.shift - a.shift;
        NormalExpr::Pos(NFOptEq { lhs, rhs, rhs_add })
    }
}
