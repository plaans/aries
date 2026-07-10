use std::ops::Not;

use aries_solver::lang::{expr::*, Store};

use crate::legacy::*;
use aries_solver::{
    model::{Label, Model},
    prelude::*,
    reif::{ReifExpr, Reifiable},
};
use itertools::*;
use smallvec::*;

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

#[derive(Copy, Clone, Debug)]
pub struct Eq(Atom, Atom);

impl Not for Eq {
    type Output = Neq;

    fn not(self) -> Self::Output {
        Neq(self.0, self.1)
    }
}
impl Not for &Eq {
    type Output = Neq;

    fn not(self) -> Neq {
        !*self
    }
}

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
                (Sym(_), Sym(_)) => int_eq(a.int_view().unwrap(), b.int_view().unwrap(), model),
                (Fixed(a), Fixed(b)) => {
                    debug_assert_eq!(a.denom, b.denom); // should be guarded by the kind comparison
                    int_eq(a.num, b.num, model)
                }
                _ => unreachable!(), // guarded by kind comparison
            }
        }
    }
}

impl Eq {
    /// Returns an equivalent *conjunction* of `ReifExpr`
    fn as_elementary_constraints<Ctx: Store>(&self, _store: &mut Ctx) -> SmallVec<[ReifExpr; 2]> {
        let a = self.0;
        let b = self.1;
        let subs: SmallVec<[ReifExpr; 2]> = if a == b {
            smallvec![]
        } else if a.kind() != b.kind() {
            panic!("Attempting to build an equality between expression with incompatible types.");
        } else {
            use Atom::*;
            match (a, b) {
                (Bool(a), Bool(b)) => {
                    smallvec![implies(a, b).into(), implies(b, a).into()]
                }
                (Int(a), Int(b)) => {
                    smallvec![leq(a, b).into(), leq(b, a).into()]
                }
                (Sym(_), Sym(_)) => {
                    let a = a.int_view().unwrap();
                    let b = b.int_view().unwrap();
                    smallvec![leq(a, b).into(), leq(b, a).into()]
                }
                (Fixed(a), Fixed(b)) => {
                    debug_assert_eq!(a.denom, b.denom); // should be guarded by the kind comparison
                    smallvec![leq(a.num, b.num).into(), leq(b.num, a.num).into()]
                }
                _ => unreachable!(), // guarded by kind comparison
            }
        };
        subs
    }
}

impl<Ctx: Store> BoolExpr<Ctx> for Eq {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        let elems = self.as_elementary_constraints(ctx);
        for elem in elems {
            elem.enforce_if(l, ctx);
        }
    }
    fn implicant(&self, ctx: &mut Ctx) -> Lit {
        let elems = self.as_elementary_constraints(ctx);
        if elems.contains(&ReifExpr::Lit(Lit::FALSE)) {
            return Lit::FALSE;
        }
        let conjuncts = elems.into_iter().map(|e| ctx.get_implicant(e)).collect_vec();
        ctx.get_implicant(and(conjuncts).into())
    }

    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        [ctx.presence(self.0), ctx.presence(self.1)].into()
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

impl Neq {
    /// Returns an equivalent *disjunction* of `ReifExpr`
    pub fn as_elementary_disjuncts<Ctx: Store>(&self, _store: &Ctx) -> SmallVec<[ReifExpr; 2]> {
        let a = self.0;
        let b = self.1;
        let subs: SmallVec<[ReifExpr; 2]> = if a == b {
            smallvec![Lit::FALSE.into()]
        } else if a.kind() != b.kind() {
            panic!("Attempting to build an equality between expression with incompatible types.");
        } else {
            use Atom::*;
            match (a, b) {
                (Bool(a), Bool(b)) => {
                    // ![(a => b) /\ (b => a)]
                    // !(a => b) \/ !(b => a)
                    // (!a & b) \/ (!b & a)
                    smallvec![and([a, !b]).into(), and([!a, b]).into()] // note: exclusive
                }
                (Int(a), Int(b)) => {
                    smallvec![lt(a, b).into(), lt(b, a).into()] // note: exclusive
                }
                (Sym(_), Sym(_)) => {
                    let a = a.int_view().unwrap();
                    let b = b.int_view().unwrap();
                    smallvec![lt(a, b).into(), lt(b, a).into()] // note: exclusive
                }
                (Fixed(a), Fixed(b)) => {
                    debug_assert_eq!(a.denom, b.denom); // should be guarded by the kind comparison
                    smallvec![lt(a.num, b.num).into(), gt(a.num, b.num).into()]
                }
                _ => unreachable!(), // guarded by kind comparison
            }
        };
        subs
    }
}

impl<Ctx: Store> BoolExpr<Ctx> for Neq {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        let elems = self.as_elementary_disjuncts(ctx);
        let disjuncts = elems.into_iter().map(|e| ctx.get_implicant(e)).collect_vec();
        or(disjuncts).enforce_if(l, ctx);
    }
    fn implicant(&self, ctx: &mut Ctx) -> Lit {
        let elems = self.as_elementary_disjuncts(ctx);
        if elems.contains(&ReifExpr::Lit(Lit::TRUE)) {
            return Lit::TRUE;
        }
        let disjuncts = elems.into_iter().map(|e| ctx.get_implicant(e)).collect_vec();
        ctx.get_implicant(or(disjuncts).into())
    }
    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        [ctx.presence(self.0), ctx.presence(self.1)].into()
    }
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

pub fn f_geq(lhs: impl Into<FAtom>, rhs: impl Into<FAtom>) -> Leq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.denom, rhs.denom);
    geq(lhs.num, rhs.num)
}

#[derive(Copy, Clone, Debug)]
pub struct Leq(IAtom, IAtom);

aries_solver::impl_reif!(Leq);

impl Not for Leq {
    type Output = Leq;

    fn not(self) -> Self::Output {
        gt(self.0, self.1)
    }
}
impl Not for &Leq {
    type Output = Leq;

    fn not(self) -> Self::Output {
        !*self
    }
}

impl From<Leq> for ReifExpr {
    fn from(value: Leq) -> Self {
        let lhs = value.0;
        let rhs = value.1;

        // normalize, transfer the shift from right to left
        // to get: lhs <= rhs + rhs_add
        let rhs_add = rhs.shift - lhs.shift;
        let lhs: Var = lhs.var;
        let rhs: Var = rhs.var;

        // Only encode as a LEQ the patterns with two variables.
        // Other are treated either are constant (if provable as so)
        // or as literals on a single variable
        if lhs == rhs {
            // X  <= X + rhs_add   <=>  0 <= rhs_add
            (0 <= rhs_add).into()
        } else if rhs == Var::ZERO {
            // lhs  <= rhs_add
            Lit::leq(lhs, rhs_add).into()
        } else if lhs == Var::ZERO {
            // 0 <= rhs + rhs_add   <=>  -rhs_add <= rhs
            Lit::geq(rhs, -rhs_add).into()
        } else {
            aries_solver::prelude::leq(lhs, rhs + rhs_add).into()
        }
    }
}

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
