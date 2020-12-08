pub mod assignments;
pub mod bool_model;
pub mod expressions;
pub mod int_model;
pub mod lang;

use crate::backtrack::Backtrack;
use crate::model::assignments::{Assignment, SavedAssignment};
use crate::queues::QReader;
use aries_sat::all::Lit;
use bool_model::*;
use expressions::*;
use int_model::*;
use lang::*;
use std::convert::TryInto;

type Label = String;

pub struct ModelEvents {
    pub bool_events: QReader<(Lit, WriterId)>,
}

#[derive(Default)]
pub struct Model {
    pub bools: BoolModel,
    pub ints: IntModel,
    pub expressions: Expressions,
    assignments: Vec<SavedAssignment>,
}

impl Model {
    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        self.bools.new_bvar(label)
    }

    pub fn new_ivar<L: Into<Label>>(&mut self, lb: IntCst, ub: IntCst, label: L) -> IVar {
        self.ints.new_ivar(lb, ub, label)
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

    // ================= Assignments =========================

    pub fn current_assignment(&self) -> &impl Assignment {
        self
    }

    pub fn save_current_assignment(&mut self, overwrite_previous: bool) {
        let ass = SavedAssignment::from_model(self);
        if overwrite_previous {
            self.assignments.pop();
        }
        self.assignments.push(ass);
    }

    pub fn last_saved_assignment(&self) -> Option<&impl Assignment> {
        self.assignments.last()
    }

    // ======= Listeners to changes in the model =======

    pub fn bool_event_reader(&self) -> QReader<(Lit, WriterId)> {
        self.bools.trail.reader()
    }

    pub fn readers(&self) -> ModelEvents {
        ModelEvents {
            bool_events: self.bool_event_reader(),
        }
    }

    // ====== Write access to the model ========

    pub fn writer(&mut self, token: WriterId) -> WModel {
        WModel { model: self, token }
    }

    // ======= Convenience method to create expressions ========

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
        !self.eq(a, b)
    }

    pub fn implies<A: Into<BAtom>, B: Into<BAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        let implication = Expr::new(Fun::Or, &[Atom::from(!a), Atom::from(b)]);
        self.intern_bool(implication).unwrap()
    }

    // =========== Formatting ==============

    /// Wraps an atom into a custom object that can be formatted with the standard library `Display`
    ///
    /// Expressions and variables are formatted into a single line with lisp-like syntax.
    /// Anonymous variables are prefixed with "b_" and "i_" (for bools and ints respectively followed
    /// by a unique identifier.
    ///
    /// # Usage
    /// ```
    /// use aries_smt::model::Model;
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

    #[allow(clippy::comparison_chain)]
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
}

/// Identifies an external writer to the model.
#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct WriterId(u8);
impl WriterId {
    pub fn new(num: impl Into<u8>) -> WriterId {
        WriterId(num.into())
    }
}

/// Provides write access to a model, making sure the built-in `WriterId` is always set.
pub struct WModel<'a> {
    model: &'a mut Model,
    token: WriterId,
}

impl<'a> WModel<'a> {
    pub fn set(&mut self, lit: Lit) {
        self.model.bools.set(lit, self.token);
    }

    pub fn set_upper_bound(&mut self, ivar: IVar, ub: IntCst) {
        self.model.ints.set_ub(ivar, ub, self.token);
    }
    pub fn set_lower_bound(&mut self, ivar: IVar, lb: IntCst) {
        self.model.ints.set_lb(ivar, lb, self.token);
    }
}

impl Backtrack for Model {
    fn save_state(&mut self) -> u32 {
        let a = self.bools.save_state();
        let b = self.ints.save_state();
        assert_eq!(a, b, "Different number of saved levels");
        a
    }

    fn num_saved(&self) -> u32 {
        assert_eq!(self.bools.num_saved(), self.ints.num_saved());
        self.bools.num_saved()
    }

    fn restore_last(&mut self) {
        self.bools.restore_last();
        self.ints.restore_last();
    }

    fn restore(&mut self, saved_id: u32) {
        self.bools.restore(saved_id);
        self.ints.restore(saved_id);
    }
}

impl Assignment for Model {
    fn literal_of(&self, bool_var: BVar) -> Option<Lit> {
        self.bools.literal_of(bool_var)
    }

    fn value_of_sat_variable(&self, sat_variable: aries_sat::all::BVar) -> Option<bool> {
        self.bools.value(sat_variable.true_lit())
    }

    fn domain_of(&self, int_var: IVar) -> &IntDomain {
        self.ints.domain_of(int_var)
    }

    fn to_owned(&self) -> SavedAssignment {
        SavedAssignment::from_model(self)
    }
}
