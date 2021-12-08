use crate::bounds::Lit;
use crate::lang::*;
use std::cmp::Ordering;

/// Normal form of an expression with component of type `X`.
/// It can be either: a literal, an expression of type `X`
/// or the negation of an expression of type `X`.
pub enum NormalExpr<X> {
    Literal(Lit),
    Pos(X),
    Neg(X),
}

impl<T> std::ops::Not for NormalExpr<T> {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            NormalExpr::Literal(l) => NormalExpr::Literal(!l),
            NormalExpr::Pos(e) => NormalExpr::Neg(e),
            NormalExpr::Neg(e) => NormalExpr::Pos(e),
        }
    }
}

impl<T> From<Lit> for NormalExpr<T> {
    fn from(l: Lit) -> Self {
        Self::Literal(l)
    }
}
impl<T> From<bool> for NormalExpr<T> {
    fn from(l: bool) -> Self {
        Self::Literal(l.into())
    }
}

/// Canonical representation of an inequality:  `lhs <= rhs + rhs_add`
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NFLeq {
    pub lhs: VarRef,
    pub rhs: VarRef,
    pub rhs_add: IntCst,
}

impl NFLeq {
    pub fn new(lhs: VarRef, rhs: VarRef, rhs_add: IntCst) -> Self {
        debug_assert!(lhs < rhs, "Violated lexical order invariant");
        debug_assert_ne!(lhs, VarRef::ZERO);
        debug_assert_ne!(rhs, VarRef::ZERO);
        NFLeq { lhs, rhs, rhs_add }
    }
    pub fn geq<A: Into<IAtom>, B: Into<IAtom>>(a: A, b: B) -> NormalExpr<Self> {
        Self::leq(b, a)
    }
    pub fn gt<A: Into<IAtom>, B: Into<IAtom>>(a: A, b: B) -> NormalExpr<Self> {
        Self::lt(b, a)
    }
    pub fn lt<A: Into<IAtom>, B: Into<IAtom>>(a: A, b: B) -> NormalExpr<Self> {
        Self::leq(a.into() + 1, b)
    }
    pub fn leq<A: Into<IAtom>, B: Into<IAtom>>(lhs: A, rhs: B) -> NormalExpr<Self> {
        let lhs = lhs.into();
        let rhs = rhs.into();

        // normalize, transfer the shift from right to left
        let rhs_add = rhs.shift - lhs.shift;
        let lhs: VarRef = lhs.var.into();
        let rhs: VarRef = rhs.var.into();

        // Only encode as a LEQ the patterns with two variables.
        // Other are treated either are constant (if provable as so)
        // or as bounds on a single variable
        if lhs == rhs {
            // X  <= X + rhs_add   <=>  0 <= rhs_add
            return (0 <= rhs_add).into();
        }
        if rhs == VarRef::ZERO {
            // lhs  <= rhs_add
            return Lit::leq(lhs, rhs_add).into();
        }
        if lhs == VarRef::ZERO {
            // 0 <= rhs + rhs_add   <=>  -rhs_add <= rhs
            return Lit::geq(rhs, -rhs_add).into();
        }

        // maintain the invariant that left side of the LEQ has a small lexical order
        match lhs.cmp(&rhs) {
            Ordering::Less => NormalExpr::Pos(Self::new(lhs, rhs, rhs_add)),
            Ordering::Equal => unreachable!(),
            Ordering::Greater => {
                // (lhs <= rhs + rhs_add) <=> (not (rhs <= lhs - rhs_add -1)
                NormalExpr::Neg(Self::new(rhs, lhs, -rhs_add - 1))
            }
        }
    }
}

/// lhs = rhs + rhs_add
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NFEq {
    pub lhs: VarRef,
    pub rhs: VarRef,
    pub rhs_add: IntCst,
}

impl NFEq {
    pub fn new(lhs: VarRef, rhs: VarRef, rhs_add: IntCst) -> Self {
        debug_assert!(lhs < rhs, "Invariant violated");
        NFEq { lhs, rhs, rhs_add }
    }

    /// Builds the normal form for this equality.
    pub fn eq<A: Into<Atom>, B: Into<Atom>>(a: A, b: B) -> NormalExpr<NFEq> {
        let a = a.into();
        let b = b.into();
        if a == b {
            Lit::TRUE.into()
        } else if a.kind() != b.kind() {
            panic!("Attempting to build an equality between expression with incompatible types.");
        } else {
            use Atom::*;
            match (a, b) {
                (Bool(_a), Bool(_b)) => todo!(),
                (Int(a), Int(b)) => Self::int_eq(a, b),
                (Sym(a), Sym(b)) => Self::int_eq(a.int_view(), b.int_view()),
                (Fixed(a), Fixed(b)) => {
                    debug_assert_eq!(a.denom, b.denom); // should be guarded by the kind comparison
                    Self::int_eq(a.num, b.num)
                }
                _ => unreachable!(), // guarded by kind comparison
            }
        }
    }

    pub fn int_eq(a: IAtom, b: IAtom) -> NormalExpr<NFEq> {
        let b_add = b.shift - a.shift;
        let a: VarRef = a.var.into();
        let b: VarRef = b.var.into();

        match a.cmp(&b) {
            Ordering::Less => NormalExpr::Pos(Self::new(a, b, b_add)),
            Ordering::Equal => (b_add == 0).into(),
            Ordering::Greater => NormalExpr::Pos(Self::new(b, a, -b_add)),
        }
    }
}

/// (lhs <= rhs + rhs_add) || absent(lhs) || absent(rhs)
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NFOptLeq {
    pub lhs: VarRef,
    pub rhs: VarRef,
    pub rhs_add: IntCst,
}

/// (lhs = rhs + rhs_add) || absent(lhs) || absent(rhs)
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NFOptEq {
    pub lhs: VarRef,
    pub rhs: VarRef,
    pub rhs_add: IntCst,
}
