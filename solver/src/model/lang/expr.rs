use crate::core::literals::Disjunction;
use crate::core::*;
use crate::model::lang::{Atom, FAtom, IAtom};
use crate::model::{Label, Model};
use crate::reif::{DifferenceExpression, ReifExpr, Reifiable};
use std::ops::Not;

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

impl From<Or> for ReifExpr {
    fn from(value: Or) -> Self {
        Disjunction::new(value.0.to_vec()).into()
    }
}

pub struct And(Box<[Lit]>);

impl From<And> for ReifExpr {
    fn from(value: And) -> Self {
        // (and a b c) <=> (not (or !a !b !c))
        let negated_literals = value.0.iter().copied().map(|l| !l).collect();
        let not_reified = ReifExpr::from(Disjunction::new(negated_literals));
        !not_reified
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Leq(IAtom, IAtom);

impl Not for Leq {
    type Output = Leq;

    fn not(self) -> Self::Output {
        gt(self.0, self.1)
    }
}

impl From<Leq> for ReifExpr {
    fn from(value: Leq) -> Self {
        let lhs = value.0;
        let rhs = value.1;

        // normalize, transfer the shift from right to left
        // to get: lhs <= rhs + rhs_add
        let rhs_add = rhs.shift - lhs.shift;
        let lhs: VarRef = lhs.var.into();
        let rhs: VarRef = rhs.var.into();

        // Only encode as a LEQ the patterns with two variables.
        // Other are treated either are constant (if provable as so)
        // or as literals on a single variable
        if lhs == rhs {
            // X  <= X + rhs_add   <=>  0 <= rhs_add
            (0 <= rhs_add).into()
        } else if rhs == VarRef::ZERO {
            // lhs  <= rhs_add
            Lit::leq(lhs, rhs_add).into()
        } else if lhs == VarRef::ZERO {
            // 0 <= rhs + rhs_add   <=>  -rhs_add <= rhs
            Lit::geq(rhs, -rhs_add).into()
        } else {
            ReifExpr::MaxDiff(DifferenceExpression::new(lhs, rhs, rhs_add))
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Eq(Atom, Atom);

impl<Lbl: Label> Reifiable<Lbl> for Eq {
    fn decompose(self, model: &mut Model<Lbl>) -> ReifExpr {
        let a = self.0;
        let b = self.1;
        if a == b {
            Lit::TRUE.into()
        } else if a.kind() != b.kind() {
            panic!("Attempting to build an equality between expression with incompatible types.");
        } else {
            use Atom::*;
            match (a, b) {
                (Bool(a), Bool(b)) => {
                    let lr = model.reify(implies(a, b));
                    let rl = model.reify(implies(b, a));
                    and([lr, rl]).into()
                }
                (Int(a), Int(b)) => int_eq(a, b, model),
                (Sym(a), Sym(b)) => int_eq(a.int_view(), b.int_view(), model),
                (Fixed(a), Fixed(b)) => {
                    debug_assert_eq!(a.denom, b.denom); // should be guarded by the kind comparison
                    int_eq(a.num, b.num, model)
                }
                _ => unreachable!(), // guarded by kind comparison
            }
        }
    }
}

fn int_eq<Lbl: Label>(a: IAtom, b: IAtom, model: &mut Model<Lbl>) -> ReifExpr {
    let lr = model.reify(leq(a, b));
    let rl = model.reify(leq(b, a));
    and([lr, rl]).into()
}

pub struct Neq(Atom, Atom);

impl<Lbl: Label> Reifiable<Lbl> for Neq {
    fn decompose(self, model: &mut Model<Lbl>) -> ReifExpr {
        !eq(self.0, self.1).decompose(model)
    }
}
