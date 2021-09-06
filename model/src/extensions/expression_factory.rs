use crate::bounds::Lit;
use crate::lang::{Atom, BAtom, BExpr, Expr, Fun, IAtom, SAtom, VarRef};
use std::cmp::Ordering;

/// Provides extension methods to allow the construction of expressions.
///
/// The implementer of this trait only requires the capability of interning expressions.
pub trait ExpressionFactoryExt {
    fn intern_bool(&mut self, expr: Expr) -> BExpr;
    fn presence_literal(&self, variable: VarRef) -> Lit;

    // ======= Convenience methods to create expressions ========

    fn or(&mut self, disjuncts: &[BAtom]) -> BAtom {
        self.or_from_iter(disjuncts.iter().copied())
    }

    fn or_from_iter(&mut self, disjuncts: impl IntoIterator<Item = BAtom>) -> BAtom {
        let mut or: Vec<BAtom> = disjuncts.into_iter().collect();
        or.sort_by(BAtom::lexical_cmp);
        or.dedup();
        let e = Expr::new(Fun::Or, or.iter().copied().map(Atom::from).collect());
        self.intern_bool(e).into()
    }

    fn and(&mut self, conjuncts: &[BAtom]) -> BAtom {
        self.and_from_iter(conjuncts.iter().copied())
    }

    fn and_from_iter(&mut self, conjuncts: impl Iterator<Item = BAtom>) -> BAtom {
        !self.or_from_iter(conjuncts.map(|b| !b))
    }

    fn and2(&mut self, a: BAtom, b: BAtom) -> BAtom {
        self.and(&[a, b])
    }
    fn or2(&mut self, a: BAtom, b: BAtom) -> BAtom {
        let and = Expr::new2(Fun::Or, a, b);
        self.intern_bool(and).into()
    }

    fn leq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let mut a = a.into();
        let mut b = b.into();

        // normalize, transfer the shift from right to left
        a.shift -= b.shift;
        b.shift = 0;

        let x = a.shift;
        // we are in the form va + X <= vb

        // only encode as a LEQ the patterns with two variables
        // other are treated either are constant (if provable as so)
        // or as bounds on a single variable
        match (a.var, b.var) {
            (None, None) => {
                // X <= 0
                return BAtom::Cst(x <= 0);
            }
            (Some(va), Some(vb)) if va == vb => {
                // va +X <= va   <=>  X <= 0
                return BAtom::Cst(x <= 0);
            }
            (Some(va), None) => {
                // va + X <= 0   <=> va <= -X
                return Lit::leq(va, -x).into();
            }
            (None, Some(vb)) => {
                // X <= vb   <=>  vb >= X
                return Lit::geq(vb, x).into();
            }
            (_, _) => {
                // general, form, continue
            }
        }

        // maintain the invariant that left side of the LEQ has a small lexical order
        match a.lexical_cmp(&b) {
            Ordering::Less => {
                let leq = Expr::new2(Fun::Leq, a, b);
                self.intern_bool(leq).into()
            }
            Ordering::Equal => true.into(),
            Ordering::Greater => {
                // swap the order by making !(b + 1 <= a)
                // normalize, transfer the shift from right to left
                b.shift -= a.shift;
                a.shift = 0;

                let leq = Expr::new2(Fun::Leq, b + 1, a);
                (!self.intern_bool(leq)).into()
            }
        }
    }

    fn geq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        self.leq(b, a)
    }

    fn lt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        self.leq(a + 1, b)
    }

    fn gt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        self.lt(b, a)
    }

    fn eq<A: Into<Atom>, B: Into<Atom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        if a == b {
            BAtom::Cst(true)
        } else if a.kind() != b.kind() {
            BAtom::Cst(false)
        } else {
            use Atom::*;
            match (a, b) {
                (Bool(_a), Bool(_b)) => todo!(),
                (Int(a), Int(b)) => self.int_eq(a, b),
                (Sym(a), Sym(b)) => self.sym_eq(a, b),
                _ => unreachable!(), // guarded by kind comparison
            }
        }
    }

    fn int_eq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let mut a = a.into();
        let mut b = b.into();

        // normalize, transfer the shift from right to left
        a.shift -= b.shift;
        b.shift = 0;

        match a.lexical_cmp(&b) {
            Ordering::Less => {
                let eq = Expr::new2(Fun::Eq, a, b);
                self.intern_bool(eq).into()
            }
            Ordering::Equal => true.into(),
            Ordering::Greater => {
                // normalize, transfer the shift from right to left
                b.shift -= a.shift;
                a.shift = 0;
                let eq = Expr::new2(Fun::Eq, b, a);
                self.intern_bool(eq).into()
            }
        }
    }

    fn sym_eq<A: Into<SAtom>, B: Into<SAtom>>(&mut self, a: A, b: B) -> BAtom {
        self.int_eq(a.into().int_view(), b.into().int_view())
    }

    fn neq<A: Into<Atom>, B: Into<Atom>>(&mut self, a: A, b: B) -> BAtom {
        !self.eq(a, b)
    }

    fn implies<A: Into<BAtom>, B: Into<BAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        let implication = Expr::new2(Fun::Or, !a, b);
        self.intern_bool(implication).into()
    }

    // =========== Optionals ===============

    /// Specifies that two optional variables must be equal if present.
    fn opt_eq<A: Into<Atom>, B: Into<Atom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        if a == b {
            BAtom::Cst(true)
        } else if a.kind() != b.kind() {
            BAtom::Cst(false)
        } else {
            use Atom::*;
            match (a, b) {
                (Bool(_a), Bool(_b)) => todo!(),
                (Int(a), Int(b)) => self.opt_int_eq(a, b),
                (Sym(a), Sym(b)) => self.opt_sym_eq(a, b),
                _ => unreachable!(), // guarded by kind comparison
            }
        }
    }

    /// Specifies that two optional int variables must be equal if present.
    fn opt_int_eq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let mut a = a.into();
        let mut b = b.into();

        // normalize, transfer the shift from right to left
        a.shift -= b.shift;
        b.shift = 0;

        match a.lexical_cmp(&b) {
            Ordering::Less => {
                let eq = Expr::new2(Fun::OptEq, a, b);
                self.intern_bool(eq).into()
            }
            Ordering::Equal => true.into(),
            Ordering::Greater => {
                // normalize, transfer the shift from right to left
                b.shift -= a.shift;
                a.shift = 0;
                let eq = Expr::new2(Fun::OptEq, b, a);
                self.intern_bool(eq).into()
            }
        }
    }

    /// Specifies that two optional symbolic variables must be equal if present.
    fn opt_sym_eq<A: Into<SAtom>, B: Into<SAtom>>(&mut self, a: A, b: B) -> BAtom {
        self.opt_int_eq(a.into().int_view(), b.into().int_view())
    }

    /// Specifies that, if the two variables are present, then a <= b
    fn opt_leq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let mut a = a.into();
        let mut b = b.into();

        // normalize, transfer the shift from right to left
        a.shift -= b.shift;
        b.shift = 0;

        let x = a.shift;
        // we are in the form va + X <= vb

        // only encode as a LEQ the patterns with two variables
        // other are treated either are constant (if provable as so)
        // or as bounds on a single variable
        match (a.var, b.var) {
            (None, None) => {
                // X <= 0
                return BAtom::Cst(x <= 0);
            }
            (Some(va), Some(vb)) if va == vb => {
                // va +X <= va   <=>  X <= 0
                return if x <= 0 {
                    // this is always true
                    BAtom::Cst(true)
                } else {
                    // base expression is always violated, so only valid
                    // if the variable is absent
                    (!self.presence_literal(va.into())).into()
                };
            }
            (Some(va), None) => {
                // va + X <= 0   <=> va <= -X
                return Lit::leq(va, -x).into();
            }
            (None, Some(vb)) => {
                // X <= vb   <=>  vb >= X
                return Lit::geq(vb, x).into();
            }
            (_, _) => {
                // general, form, continue
            }
        }

        // maintain the invariant that left side of the LEQ has a small lexical order
        match a.lexical_cmp(&b) {
            Ordering::Less => {
                let leq = Expr::new2(Fun::OptLeq, a, b);
                self.intern_bool(leq).into()
            }
            Ordering::Equal => true.into(),
            Ordering::Greater => {
                // swap the order by making !(b + 1 <= a)
                // normalize, transfer the shift from right to left
                b.shift -= a.shift;
                a.shift = 0;

                let leq = Expr::new2(Fun::OptLeq, b + 1, a);
                (!self.intern_bool(leq)).into()
            }
        }
    }
}
