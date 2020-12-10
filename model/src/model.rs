use crate::assignments::{Assignment, SavedAssignment};
use crate::bool_model::*;
use crate::expressions::*;
use crate::int_model::*;
use crate::lang::*;
use aries_backtrack::Backtrack;
use aries_backtrack::QReader;
use aries_sat::all::Lit;

use crate::symbols::SymbolTable;
use crate::types::TypeId;
use crate::Label;
use aries_collections::ref_store::RefMap;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::sync::Arc;

pub struct ModelEvents {
    pub bool_events: QReader<(Lit, WriterId)>,
}

pub struct Model {
    symbols: Arc<SymbolTable<String, String>>,
    pub bools: BoolModel,
    pub ints: IntModel,
    pub types: RefMap<DVar, DiscreteType>,
    pub expressions: Expressions,
    assignments: Vec<SavedAssignment>,
}

impl Model {
    pub fn new() -> Self {
        Self::new_with_symbols(Arc::new(SymbolTable::empty()))
    }

    pub fn new_with_symbols(symbols: Arc<SymbolTable<String, String>>) -> Self {
        Model {
            symbols,
            bools: Default::default(),
            ints: Default::default(),
            types: Default::default(),
            expressions: Default::default(),
            assignments: vec![],
        }
    }

    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        self.bools.new_bvar(label)
    }

    pub fn new_ivar(&mut self, lb: IntCst, ub: IntCst, label: impl Into<Label>) -> IVar {
        let dvar = self.ints.new_ivar(lb, ub, label);
        self.types.insert(dvar, DiscreteType::integer());
        IVar::new(dvar)
    }

    pub fn new_sym_var(&mut self, tpe: TypeId, label: impl Into<Label>) -> SVar {
        let instances = self.symbols.instances_of_type(tpe);
        let dvar = match instances.bounds() {
            Some((lb, ub)) => {
                let lb = usize::from(lb) as IntCst;
                let ub = usize::from(ub) as IntCst;
                self.ints.new_ivar(lb, ub, label)
            }
            None => {
                // no instances for this type, make a variable with empty domain
                self.ints.new_ivar(1, 0, label)
            }
        };
        self.types.insert(dvar, DiscreteType::new_symbolic(tpe));
        SVar::new(dvar, tpe)
    }

    pub fn bounds(&self, ivar: IVar) -> (IntCst, IntCst) {
        let IntDomain { lb, ub, .. } = self.ints.domain_of(ivar);
        (*lb, *ub)
    }

    pub fn expr_of(&self, atom: impl Into<BAtom>) -> Option<NExpr> {
        self.expressions.expr_of(atom)
    }

    pub fn intern_bool(&mut self, e: Expr) -> BVar {
        match self.expressions.variable_of(&e) {
            Some(variable) => {
                assert_eq!(
                    &e,
                    self.expressions
                        .get(self.expressions.expr_of_variable(variable).unwrap())
                );
                variable
            }
            None => {
                let key = self.new_bvar("");
                self.expressions.bind(key, e);
                key
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

    // ======= Convenience methods to create expressions ========

    pub fn or(&mut self, disjuncts: &[BAtom]) -> BAtom {
        self.or_from_iter(disjuncts.iter().copied())
    }

    pub fn or_from_iter(&mut self, disjuncts: impl IntoIterator<Item = BAtom>) -> BAtom {
        let mut or: Vec<BAtom> = disjuncts.into_iter().collect();
        or.sort_by(BAtom::lexical_cmp);
        or.dedup();
        let e = Expr::new(Fun::Or, or.iter().copied().map(Atom::from).collect());
        self.intern_bool(e).into()
    }

    pub fn and(&mut self, conjuncts: &[BAtom]) -> BAtom {
        self.and_from_iter(conjuncts.iter().copied())
    }

    pub fn and_from_iter(&mut self, conjuncts: impl Iterator<Item = BAtom>) -> BAtom {
        !self.or_from_iter(conjuncts.map(|b| !b))
    }

    pub fn and2(&mut self, a: BAtom, b: BAtom) -> BAtom {
        self.and(&[a, b])
    }
    pub fn or2(&mut self, a: BAtom, b: BAtom) -> BAtom {
        let and = Expr::new2(Fun::Or, a, b);
        self.intern_bool(and).into()
    }

    pub fn leq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        // maintain the invariant that left side of the LEQ has a small lexical order
        match a.lexical_cmp(&b) {
            Ordering::Less => {
                let leq = Expr::new2(Fun::Leq, a, b);
                self.intern_bool(leq).into()
            }
            Ordering::Equal => true.into(),
            Ordering::Greater => {
                // swap the order by making !(b + 1 <= a)
                let lt = Expr::new2(Fun::Leq, b + 1, a);
                !BAtom::from(self.intern_bool(lt))
            }
        }
    }

    pub fn geq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        self.leq(b, a)
    }

    pub fn lt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        self.leq(a + 1, b)
    }

    pub fn gt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        !self.lt(b, a)
    }

    pub fn eq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        match a.lexical_cmp(&b) {
            Ordering::Less => {
                let eq = Expr::new2(Fun::Eq, a, b);
                self.intern_bool(eq).into()
            }
            Ordering::Equal => true.into(),
            Ordering::Greater => {
                let eq = Expr::new2(Fun::Eq, b, a);
                self.intern_bool(eq).into()
            }
        }
    }

    pub fn neq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        !self.eq(a, b)
    }

    pub fn implies<A: Into<BAtom>, B: Into<BAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        let implication = Expr::new2(Fun::Or, !a, b);
        self.intern_bool(implication).into()
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
    /// use aries_model::Model;
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
        match BAtom::try_from(atom).ok().and_then(|batom| self.expr_of(batom)) {
            Some(NExpr::Pos(e)) => {
                write!(f, "({}", e.fun)?;
                for arg in &e.args {
                    write!(f, " ")?;
                    self.format_impl(*arg, f)?;
                }
                write!(f, ")")
            }
            Some(NExpr::Neg(e)) => {
                write!(f, "(not ({}", e.fun)?;
                for arg in &e.args {
                    write!(f, " ")?;
                    self.format_impl(*arg, f)?;
                }
                write!(f, "))")
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
                Atom::Disc(d) => {
                    let i = IAtom::try_from(d).expect("TODO: NOT IMPLEMENTED FOR SYMBOLS");
                    match i.var {
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
                                write!(f, "i_{}", usize::from(DVar::from(v)))?;
                            }
                            if i.shift != 0 {
                                write!(f, " {})", i.shift.abs())?;
                            }
                            std::fmt::Result::Ok(())
                        }
                    }
                }
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

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
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

    fn var_domain(&self, var: impl Into<DVar>) -> &IntDomain {
        self.ints.domain_of(var.into())
    }

    fn to_owned(&self) -> SavedAssignment {
        SavedAssignment::from_model(self)
    }
}
