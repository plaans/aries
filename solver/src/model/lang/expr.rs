use crate::core::literals::{Disjunction, Lits};
use crate::core::*;
use crate::model::lang::alternative::Alternative;
use crate::model::lang::hreif::{BoolExpr, Store};
use crate::model::lang::{Atom, FAtom, IAtom, SAtom};
use crate::model::{Label, Model};
use crate::reif::{DifferenceExpression, ReifExpr, Reifiable};
use env_param::EnvParam;
use itertools::Itertools;
use smallvec::{SmallVec, smallvec};
use std::ops::Not;

use super::IVar;
use super::mul::EqMul;

static USE_EQUALITY_LOGIC: EnvParam<bool> = EnvParam::new("ARIES_USE_EQ_LOGIC", "false");

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

pub fn f_geq(lhs: impl Into<FAtom>, rhs: impl Into<FAtom>) -> Leq {
    let lhs = lhs.into();
    let rhs = rhs.into();
    assert_eq!(lhs.denom, rhs.denom);
    geq(lhs.num, rhs.num)
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

pub fn or(disjuncts: impl Into<Disjunction>) -> Or {
    disjuncts.into()
}
pub fn and(disjuncts: impl Into<Box<[Lit]>>) -> And {
    And(disjuncts.into())
}
pub fn implies(a: impl Into<Lit>, b: impl Into<Lit>) -> Or {
    or([!a.into(), b.into()])
}

/// Creates a new expression that is true iff `lhs = factor1 * factor2`
pub fn eq_mul(lhs: impl Into<IVar>, factor1: impl Into<IVar>, factor2: impl Into<IVar>) -> EqMul {
    EqMul::new(lhs.into(), factor1.into(), factor2.into())
}

pub fn alternative<T: Into<Atom>>(main: impl Into<Atom>, alternatives: impl IntoIterator<Item = T>) -> Alternative {
    Alternative::new(main, alternatives)
}

pub type Or = Disjunction;

impl Not for Or {
    type Output = And;

    fn not(self) -> Self::Output {
        let mut lits = self.into_lits();
        lits.iter_mut().for_each(|l| *l = !*l);
        And(lits.into_boxed_slice())
    }
}

#[derive(Clone)]
pub struct And(Box<[Lit]>);

impl Not for And {
    type Output = Or;

    fn not(self) -> Self::Output {
        let mut lits = self.0;
        lits.iter_mut().for_each(|l| *l = !*l);
        Disjunction::from_vec(lits.to_vec())
    }
}

impl From<And> for ReifExpr {
    fn from(value: And) -> Self {
        // (and a b c) <=> (not (or !a !b !c))
        let negated_literals: Lits = value.0.iter().copied().map(|l| !l).collect();
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
                (Sym(_), Sym(_)) if !USE_EQUALITY_LOGIC.get() => {
                    int_eq(a.int_view().unwrap(), b.int_view().unwrap(), model)
                }
                (Sym(va), Sym(vb)) => match (va, vb) {
                    (SAtom::Var(a), SAtom::Var(b)) => {
                        if a.var <= b.var {
                            ReifExpr::Eq(a.var, b.var)
                        } else {
                            ReifExpr::Eq(b.var, a.var)
                        }
                    }
                    (SAtom::Cst(a), SAtom::Cst(b)) => {
                        let l = if a == b { Lit::TRUE } else { Lit::FALSE };
                        ReifExpr::Lit(l)
                    }
                    (SAtom::Var(x), SAtom::Cst(v)) | (SAtom::Cst(v), SAtom::Var(x)) => {
                        let var = x.var;
                        let value = v.sym.int_value();
                        let (lb, ub) = model.state.bounds(var);
                        if (lb..=ub).contains(&value) {
                            ReifExpr::EqVal(x.var, v.sym.int_value())
                        } else {
                            ReifExpr::Lit(Lit::FALSE)
                        }
                    }
                },
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
    fn as_elementary_constraints(&self, store: &dyn Store) -> SmallVec<[ReifExpr; 2]> {
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
                (Sym(_), Sym(_)) if !USE_EQUALITY_LOGIC.get() => {
                    let a = a.int_view().unwrap();
                    let b = b.int_view().unwrap();
                    smallvec![leq(a, b).into(), leq(b, a).into()]
                }
                (Sym(va), Sym(vb)) => match (va, vb) {
                    (SAtom::Var(a), SAtom::Var(b)) => {
                        if a.var <= b.var {
                            smallvec![ReifExpr::Eq(a.var, b.var)]
                        } else {
                            smallvec![ReifExpr::Eq(b.var, a.var)]
                        }
                    }
                    (SAtom::Cst(a), SAtom::Cst(b)) => {
                        let l2 = if a == b { Lit::TRUE } else { Lit::FALSE };
                        smallvec![l2.into()]
                    }
                    (SAtom::Var(x), SAtom::Cst(v)) | (SAtom::Cst(v), SAtom::Var(x)) => {
                        let var = x.var;
                        let value = v.sym.int_value();
                        let (lb, ub) = store.bounds(var);
                        if (lb..=ub).contains(&value) {
                            smallvec![ReifExpr::EqVal(x.var, v.sym.int_value())]
                        } else {
                            smallvec![Lit::FALSE.into()]
                        }
                    }
                },
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

impl<Ctx> BoolExpr<Ctx> for Eq {
    fn enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store) {
        let elems = self.as_elementary_constraints(store);
        for elem in elems {
            elem.enforce_if(l, ctx, store);
        }
    }
    fn implicant(&self, _ctx: &Ctx, store: &mut dyn Store) -> Lit {
        let elems = self.as_elementary_constraints(store);
        if elems.contains(&ReifExpr::Lit(Lit::FALSE)) {
            return Lit::FALSE;
        }
        let conjuncts = elems.into_iter().map(|e| store.get_implicant(e)).collect_vec();
        store.get_implicant(and(conjuncts).into())
    }

    fn conj_scope(&self, _ctx: &Ctx, store: &dyn Store) -> super::hreif::Lits {
        smallvec::smallvec![
            store.presence_of_var(self.0.variable()),
            store.presence_of_var(self.1.variable())
        ]
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
    pub fn as_elementary_disjuncts(&self, store: &dyn Store) -> SmallVec<[ReifExpr; 2]> {
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
                (Sym(_), Sym(_)) if !USE_EQUALITY_LOGIC.get() => {
                    let a = a.int_view().unwrap();
                    let b = b.int_view().unwrap();
                    smallvec![lt(a, b).into(), lt(b, a).into()] // note: exclusive
                }
                (Sym(va), Sym(vb)) => match (va, vb) {
                    (SAtom::Var(a), SAtom::Var(b)) => {
                        if a.var <= b.var {
                            smallvec![ReifExpr::Neq(a.var, b.var)]
                        } else {
                            smallvec![ReifExpr::Neq(b.var, a.var)]
                        }
                    }
                    (SAtom::Cst(a), SAtom::Cst(b)) => {
                        let l2 = if a != b { Lit::TRUE } else { Lit::FALSE };
                        smallvec![ReifExpr::Lit(l2)]
                    }
                    (SAtom::Var(x), SAtom::Cst(v)) | (SAtom::Cst(v), SAtom::Var(x)) => {
                        let var = x.var;
                        let value = v.sym.int_value();
                        let (lb, ub) = store.bounds(var);
                        if (lb..=ub).contains(&value) {
                            smallvec![ReifExpr::NeqVal(x.var, v.sym.int_value())]
                        } else {
                            smallvec![ReifExpr::Lit(Lit::TRUE)]
                        }
                    }
                },
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

impl<Ctx> BoolExpr<Ctx> for Neq {
    fn enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store) {
        let elems = self.as_elementary_disjuncts(store);
        let disjuncts = elems.into_iter().map(|e| store.get_implicant(e)).collect_vec();
        or(disjuncts).enforce_if(l, ctx, store);
    }
    fn implicant(&self, _ctx: &Ctx, store: &mut dyn Store) -> Lit {
        let elems = self.as_elementary_disjuncts(store);
        if elems.contains(&ReifExpr::Lit(Lit::TRUE)) {
            return Lit::TRUE;
        }
        let disjuncts = elems.into_iter().map(|e| store.get_implicant(e)).collect_vec();
        store.get_implicant(or(disjuncts).into())
    }
    fn conj_scope(&self, _ctx: &Ctx, store: &dyn Store) -> super::hreif::Lits {
        smallvec::smallvec![
            store.presence_of_var(self.0.variable()),
            store.presence_of_var(self.1.variable())
        ]
    }
}
