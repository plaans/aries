use crate::core::literals::Disjunction;
use crate::core::*;
use crate::model::lang::alternative::Alternative;
use crate::model::lang::hreif::{exclu_choice, HReif};
use crate::model::lang::{Atom, FAtom, IAtom, SAtom};
use crate::model::{Label, Model};
use crate::reif::{DifferenceExpression, ReifExpr, Reifiable};
use env_param::EnvParam;
use std::ops::Not;

use super::mul::EqMul;
use super::IVar;

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

pub fn or(disjuncts: impl Into<Box<[Lit]>>) -> Or {
    Or(disjuncts.into())
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

#[derive(Clone)]
pub struct Or(Box<[Lit]>);

impl From<Or> for ReifExpr {
    fn from(value: Or) -> Self {
        Disjunction::new(value.0.to_vec()).into()
    }
}

impl Not for Or {
    type Output = And;

    fn not(self) -> Self::Output {
        let mut lits = self.0;
        lits.iter_mut().for_each(|l| *l = !*l);
        And(lits)
    }
}

#[derive(Clone)]
pub struct And(Box<[Lit]>);

impl Not for And {
    type Output = Or;

    fn not(self) -> Self::Output {
        let mut lits = self.0;
        lits.iter_mut().for_each(|l| *l = !*l);
        Or(lits)
    }
}

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

impl HReif for Eq {
    fn enforce_if(&self, l: Lit, store: &mut dyn super::hreif::Store) {
        //fn decompose(self, model: &mut Model<Lbl>) -> ReifExpr {
        let a = self.0;
        let b = self.1;
        if a == b {
            Lit::TRUE.enforce_if(l, store);
        } else if a.kind() != b.kind() {
            panic!("Attempting to build an equality between expression with incompatible types.");
        } else {
            use Atom::*;
            match (a, b) {
                (Bool(a), Bool(b)) => {
                    implies(a, b).enforce_if(l, store);
                    implies(b, a).enforce_if(l, store);
                }
                (Int(a), Int(b)) => {
                    leq(a, b).enforce_if(l, store);
                    leq(b, a).enforce_if(l, store);
                }
                (Sym(_), Sym(_)) if !USE_EQUALITY_LOGIC.get() => {
                    let a = a.int_view().unwrap();
                    let b = b.int_view().unwrap();
                    leq(a, b).enforce_if(l, store);
                    leq(b, a).enforce_if(l, store);
                }
                (Sym(va), Sym(vb)) => match (va, vb) {
                    (SAtom::Var(a), SAtom::Var(b)) => {
                        if a.var <= b.var {
                            ReifExpr::Eq(a.var, b.var).enforce_if(l, store);
                        } else {
                            ReifExpr::Eq(b.var, a.var).enforce_if(l, store);
                        }
                    }
                    (SAtom::Cst(a), SAtom::Cst(b)) => {
                        let l2 = if a == b { Lit::TRUE } else { Lit::FALSE };
                        ReifExpr::Lit(l2).enforce_if(l, store);
                    }
                    (SAtom::Var(x), SAtom::Cst(v)) | (SAtom::Cst(v), SAtom::Var(x)) => {
                        let var = x.var;
                        let value = v.sym.int_value();
                        let (lb, ub) = store.bounds(var);
                        if (lb..=ub).contains(&value) {
                            ReifExpr::EqVal(x.var, v.sym.int_value()).enforce_if(l, store);
                        } else {
                            ReifExpr::Lit(Lit::FALSE).enforce_if(l, store);
                        }
                    }
                },
                (Fixed(a), Fixed(b)) => {
                    debug_assert_eq!(a.denom, b.denom); // should be guarded by the kind comparison
                    leq(a.num, b.num).enforce_if(l, store);
                    leq(b.num, a.num).enforce_if(l, store);
                }
                _ => unreachable!(), // guarded by kind comparison
            }
        }
    }

    fn conj_scope(&self, prez: &dyn Fn(VarRef) -> Lit) -> super::hreif::Lits {
        smallvec::smallvec![prez(self.0.variable()), prez(self.1.variable())]
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

impl HReif for Neq {
    fn enforce_if(&self, l: Lit, store: &mut dyn super::hreif::Store) {
        let a = self.0;
        let b = self.1;
        if a == b {
            Lit::FALSE.enforce_if(l, store);
        } else if a.kind() != b.kind() {
            panic!("Attempting to build an equality between expression with incompatible types.");
        } else {
            use Atom::*;
            match (a, b) {
                (Bool(a), Bool(b)) => {
                    // ![(a => b) /\ (b => a)]
                    // !(a => b) \/ !(b => a)
                    // (!a & b) \/ (!b & a) // TODO strange that we cannot have a conjunction
                    exclu_choice(and([a, !b]), and([!a, b])).enforce_if(l, store);
                }
                (Int(a), Int(b)) => {
                    exclu_choice(lt(a, b), lt(b, a)).opt_enforce_if(l, store);
                }
                (Sym(_), Sym(_)) if !USE_EQUALITY_LOGIC.get() => {
                    let a = a.int_view().unwrap();
                    let b = b.int_view().unwrap();
                    exclu_choice(lt(a, b), lt(b, a)).opt_enforce_if(l, store);
                }
                (Sym(va), Sym(vb)) => match (va, vb) {
                    (SAtom::Var(a), SAtom::Var(b)) => {
                        if a.var <= b.var {
                            ReifExpr::Neq(a.var, b.var).enforce_if(l, store);
                        } else {
                            ReifExpr::Neq(b.var, a.var).enforce_if(l, store);
                        }
                    }
                    (SAtom::Cst(a), SAtom::Cst(b)) => {
                        let l2 = if a != b { Lit::TRUE } else { Lit::FALSE };
                        ReifExpr::Lit(l2).enforce_if(l, store);
                    }
                    (SAtom::Var(x), SAtom::Cst(v)) | (SAtom::Cst(v), SAtom::Var(x)) => {
                        let var = x.var;
                        let value = v.sym.int_value();
                        let (lb, ub) = store.bounds(var);
                        if (lb..=ub).contains(&value) {
                            ReifExpr::NeqVal(x.var, v.sym.int_value()).enforce_if(l, store);
                        } else {
                            ReifExpr::Lit(Lit::TRUE).enforce_if(l, store);
                        }
                    }
                },
                (Fixed(a), Fixed(b)) => {
                    debug_assert_eq!(a.denom, b.denom); // should be guarded by the kind comparison
                    exclu_choice(lt(a.num, b.num), gt(a.num, b.num)).opt_enforce_if(l, store);
                }
                _ => unreachable!(), // guarded by kind comparison
            }
        }
    }
    fn conj_scope(&self, prez: &dyn Fn(VarRef) -> Lit) -> super::hreif::Lits {
        smallvec::smallvec![prez(self.0.variable()), prez(self.1.variable())]
    }
}
