use crate::bounds::Lit;
use crate::lang::{Atom, Expr, Fun, IAtom, IVar, SAtom, VarRef};
use std::cmp::Ordering;

/// Provides extension methods to allow the construction of expressions.
///
/// The implementer of this trait only requires the capability of interning expressions.
pub trait ExpressionFactoryExt {
    fn intern_bool(&mut self, expr: Expr) -> Lit;
    fn presence_literal(&self, variable: VarRef) -> Lit;

    // ======= Convenience methods to create expressions ========

    fn or_reif(&mut self, disjuncts: &[Lit]) -> Lit {
        self.intern_bool(self.or(disjuncts)).into()
    }

    fn or(&self, disjuncts: &[Lit]) -> Expr {
        self.or_from_iter(disjuncts.iter().copied())
    }

    fn or_from_iter(&self, disjuncts: impl IntoIterator<Item = Lit>) -> Expr {
        let mut or: Vec<Lit> = disjuncts.into_iter().collect();
        or.sort_by(Lit::lexical_cmp);
        or.dedup();
        Expr::new(Fun::Or, or.iter().copied().map(Atom::from).collect())
    }

    fn and(&mut self, conjuncts: &[Lit]) -> Lit {
        self.and_from_iter(conjuncts.iter().copied())
    }

    fn and_from_iter(&mut self, conjuncts: impl Iterator<Item = Lit>) -> Lit {
        !self.intern_bool(self.or_from_iter(conjuncts.map(|b| !b)))
    }

    fn and2(&mut self, a: Lit, b: Lit) -> Lit {
        self.and(&[a, b])
    }
    fn or2(&mut self, a: Lit, b: Lit) -> Expr {
        Expr::new2(Fun::Or, a, b)
    }

    fn leq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> Lit {
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
        if a.var == b.var {
            // a.var +X <= a.var   <=>  X <= 0
            return (x <= 0).into();
        }
        if b.var == IVar::ZERO {
            // a.var + X <= 0   <=> a.var <= -X
            return Lit::leq(a.var, -x);
        }
        if a.var == IVar::ZERO {
            // X <= b.var   <=>  b.var >= X
            return Lit::geq(b.var, x);
        }

        // maintain the invariant that left side of the LEQ has a small lexical order
        match a.lexical_cmp(&b) {
            Ordering::Less => self.intern_bool(Expr::new2(Fun::Leq, a, b)),
            Ordering::Equal => true.into(),
            Ordering::Greater => {
                // swap the order by making !(b + 1 <= a)
                // normalize, transfer the shift from right to left
                b.shift -= a.shift;
                a.shift = 0;

                let leq = Expr::new2(Fun::Leq, b + 1, a);
                !self.intern_bool(leq)
            }
        }
    }

    fn geq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> Lit {
        self.leq(b, a)
    }

    fn lt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> Lit {
        let a = a.into();
        let b = b.into();
        self.leq(a + 1, b)
    }

    fn gt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> Lit {
        self.lt(b, a)
    }

    fn eq<A: Into<Atom>, B: Into<Atom>>(&self, a: A, b: B) -> Expr {
        let a = a.into();
        let b = b.into();
        if a == b {
            Expr::TRUE()
        } else if a.kind() != b.kind() {
            Expr::FALSE()
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

    fn int_eq<A: Into<IAtom>, B: Into<IAtom>>(&self, a: A, b: B) -> Expr {
        let mut a = a.into();
        let mut b = b.into();

        // normalize, transfer the shift from right to left
        a.shift -= b.shift;
        b.shift = 0;

        match a.lexical_cmp(&b) {
            Ordering::Less => Expr::new2(Fun::Eq, a, b),
            Ordering::Equal => true.into(),
            Ordering::Greater => {
                // normalize, transfer the shift from right to left
                b.shift -= a.shift;
                a.shift = 0;
                Expr::new2(Fun::Eq, b, a)
            }
        }
    }

    fn sym_eq<A: Into<SAtom>, B: Into<SAtom>>(&self, a: A, b: B) -> Expr {
        self.int_eq(a.into().int_view(), b.into().int_view())
    }

    fn neq<A: Into<Atom>, B: Into<Atom>>(&mut self, a: A, b: B) -> Lit {
        !self.intern_bool(self.eq(a, b))
    }

    fn implies<A: Into<Lit>, B: Into<Lit>>(&self, a: A, b: B) -> Expr {
        let a = a.into();
        let b = b.into();
        Expr::new2(Fun::Or, !a, b)
    }

    // =========== Optionals ===============

    /// Specifies that two optional variables must be equal if present.
    fn opt_eq<A: Into<Atom>, B: Into<Atom>>(&self, a: A, b: B) -> Expr {
        let a = a.into();
        let b = b.into();
        if a == b {
            Expr::TRUE()
        } else if a.kind() != b.kind() {
            Expr::FALSE()
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
    fn opt_int_eq<A: Into<IAtom>, B: Into<IAtom>>(&self, a: A, b: B) -> Expr {
        let mut a = a.into();
        let mut b = b.into();

        // normalize, transfer the shift from right to left
        a.shift -= b.shift;
        b.shift = 0;

        match a.lexical_cmp(&b) {
            Ordering::Less => Expr::new2(Fun::OptEq, a, b),
            Ordering::Equal => Expr::TRUE(),
            Ordering::Greater => {
                // normalize, transfer the shift from right to left
                b.shift -= a.shift;
                a.shift = 0;
                Expr::new2(Fun::OptEq, b, a)
            }
        }
    }

    /// Specifies that two optional symbolic variables must be equal if present.
    fn opt_sym_eq<A: Into<SAtom>, B: Into<SAtom>>(&self, a: A, b: B) -> Expr {
        self.opt_int_eq(a.into().int_view(), b.into().int_view())
    }

    /// Specifies that, if the two variables are present, then a <= b
    fn opt_leq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> Lit {
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
        if a.var == b.var {
            // va +X <= va   <=>  X <= 0
            return if x <= 0 {
                // this is always true
                Lit::TRUE
            } else {
                // base expression is always violated, so only valid
                // if the variable is absent
                !self.presence_literal(a.var.into())
            };
        }
        if b.var == IVar::ZERO {
            // va + X <= 0   <=> va <= -X
            return Lit::leq(a.var, -x);
        }
        if a.var == IVar::ZERO {
            // X <= vb   <=>  vb >= X
            return Lit::geq(b.var, x);
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
