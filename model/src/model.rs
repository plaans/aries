use crate::assignments::{Assignment, SavedAssignment};
use crate::expressions::*;
use crate::int_model::*;
use crate::lang::*;
use aries_backtrack::{Backtrack, DecLvl, ObsTrail};

use crate::symbols::SymbolTable;
use crate::types::TypeId;
use crate::Label;
use aries_collections::ref_store::RefMap;
use std::cmp::Ordering;

use crate::bounds::{Bound, Relation};
use crate::int_model::domains::Event;
use aries_utils::Fmt;
use std::sync::Arc;

pub struct Model {
    pub symbols: Arc<SymbolTable>,
    pub discrete: DiscreteModel,
    pub types: RefMap<VarRef, Type>,
    pub var_presence: RefMap<VarRef, BAtom>,
    pub expressions: Expressions,
    assignments: Vec<SavedAssignment>,
    num_writers: u8,
}

impl Model {
    pub fn new() -> Self {
        Self::new_with_symbols(Arc::new(SymbolTable::empty()))
    }

    pub fn new_with_symbols(symbols: Arc<SymbolTable>) -> Self {
        Model {
            symbols,
            discrete: DiscreteModel::new(),
            types: Default::default(),
            var_presence: Default::default(),
            expressions: Default::default(),
            assignments: vec![],
            num_writers: 0,
        }
    }

    pub fn new_write_token(&mut self) -> WriterId {
        self.num_writers += 1;
        WriterId(self.num_writers - 1)
    }

    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        BVar::new(self.discrete.new_var(0, 1, label))
    }

    pub fn new_ivar(&mut self, lb: IntCst, ub: IntCst, label: impl Into<Label>) -> IVar {
        self.create_ivar(lb, ub, None, label)
    }

    pub fn new_optional_ivar(
        &mut self,
        lb: IntCst,
        ub: IntCst,
        presence: impl Into<BAtom>,
        label: impl Into<Label>,
    ) -> IVar {
        self.create_ivar(lb, ub, Some(presence.into()), label)
    }

    fn create_ivar(&mut self, lb: IntCst, ub: IntCst, presence: Option<BAtom>, label: impl Into<Label>) -> IVar {
        let dvar = self.discrete.new_var(lb, ub, label);
        self.types.insert(dvar, Type::Int);
        if let Some(presence) = presence {
            self.var_presence.insert(dvar, presence);
            todo!("Use support of optionals");
        }
        IVar::new(dvar)
    }

    pub fn new_sym_var(&mut self, tpe: TypeId, label: impl Into<Label>) -> SVar {
        self.create_sym_var(tpe, None, label)
    }

    pub fn new_optional_sym_var(&mut self, tpe: TypeId, presence: impl Into<BAtom>, label: impl Into<Label>) -> SVar {
        self.create_sym_var(tpe, Some(presence.into()), label)
    }

    fn create_sym_var(&mut self, tpe: TypeId, presence: Option<BAtom>, label: impl Into<Label>) -> SVar {
        let instances = self.symbols.instances_of_type(tpe);
        let dvar = match instances.bounds() {
            Some((lb, ub)) => {
                let lb = usize::from(lb) as IntCst;
                let ub = usize::from(ub) as IntCst;
                self.discrete.new_var(lb, ub, label)
            }
            None => {
                // no instances for this type, make a variable with empty domain
                self.discrete.new_var(1, 0, label)
            }
        };
        self.types.insert(dvar, Type::Sym(tpe));
        if let Some(presence) = presence {
            self.var_presence.insert(dvar, presence);
            todo!("Use support of optional in domains")
        }
        SVar::new(dvar, tpe)
    }

    pub fn unifiable(&self, a: impl Into<Atom>, b: impl Into<Atom>) -> bool {
        let a = a.into();
        let b = b.into();
        if a.kind() != b.kind() {
            false
        } else {
            let (l1, u1) = self.int_bounds(a);
            let (l2, u2) = self.int_bounds(b);
            let disjoint = u1 < l2 || u2 < l1;
            !disjoint
        }
    }

    pub fn unifiable_seq<A: Into<Atom> + Copy, B: Into<Atom> + Copy>(&self, a: &[A], b: &[B]) -> bool {
        if a.len() != b.len() {
            false
        } else {
            for (a, b) in a.iter().zip(b.iter()) {
                let a = (*a).into();
                let b = (*b).into();
                if !self.unifiable(a, b) {
                    return false;
                }
            }
            true
        }
    }

    pub fn bounds(&self, ivar: IVar) -> (IntCst, IntCst) {
        self.discrete.domain_of(ivar)
    }

    pub fn intern_bool(&mut self, e: Expr) -> BExpr {
        let handle = self.expressions.intern(e);
        BExpr {
            expr: handle,
            negated: false,
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
                return Bound::leq(va, -x).into();
            }
            (None, Some(vb)) => {
                // X <= vb   <=>  vb >= X
                return Bound::geq(vb, x).into();
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

    pub fn geq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        self.leq(b, a)
    }

    pub fn lt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        let a = a.into();
        let b = b.into();
        self.leq(a + 1, b)
    }

    pub fn gt<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
        self.lt(b, a)
    }

    pub fn eq<A: Into<Atom>, B: Into<Atom>>(&mut self, a: A, b: B) -> BAtom {
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

    pub fn int_eq<A: Into<IAtom>, B: Into<IAtom>>(&mut self, a: A, b: B) -> BAtom {
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

    pub fn sym_eq<A: Into<SAtom>, B: Into<SAtom>>(&mut self, a: A, b: B) -> BAtom {
        self.int_eq(a.into().int_view(), b.into().int_view())
    }

    pub fn neq<A: Into<Atom>, B: Into<Atom>>(&mut self, a: A, b: B) -> BAtom {
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
        let atom = atom.into();
        Fmt(move |f| self.format_impl(atom, f))
    }

    #[allow(clippy::comparison_chain)]
    fn format_impl(&self, atom: Atom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match atom {
            Atom::Bool(b) => self.format_impl_bool(b, f),
            Atom::Int(i) => self.format_impl_int(i, f),
            Atom::Sym(s) => self.format_impl_sym(s, f),
        }
    }
    fn format_impl_bool(&self, atom: BAtom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match atom {
            BAtom::Cst(b) => write!(f, "{}", b),
            BAtom::Bound(b) => {
                self.format_impl_var(b.variable(), Kind::Int, f)?;
                write!(f, " {} {}", b.relation(), b.value())
            }
            BAtom::Expr(BExpr { expr, negated }) => {
                if negated {
                    write!(f, "(not ")?;
                }
                self.format_impl_expr(expr, f)?;
                if negated {
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    }

    fn format_impl_expr(&self, expr: ExprHandle, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let expr = self.expressions.get(expr);
        write!(f, "({}", expr.fun)?;
        for arg in &expr.args {
            write!(f, " ")?;
            self.format_impl(*arg, f)?;
        }
        write!(f, ")")
    }

    #[allow(clippy::comparison_chain)]
    fn format_impl_int(&self, i: IAtom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match i.var {
            None => write!(f, "{}", i.shift),
            Some(v) => {
                if i.shift > 0 {
                    write!(f, "(+ ")?;
                } else if i.shift < 0 {
                    write!(f, "(- ")?;
                }
                self.format_impl_var(v.into(), Kind::Int, f)?;
                if i.shift != 0 {
                    write!(f, " {})", i.shift.abs())?;
                }
                std::fmt::Result::Ok(())
            }
        }
    }

    fn format_impl_sym(&self, atom: SAtom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match atom {
            SAtom::Var(v) => self.format_impl_var(v.var, Kind::Sym, f),
            SAtom::Cst(s) => write!(f, "{}", self.symbols.symbol(s.sym)),
        }
    }

    fn format_impl_var(&self, v: VarRef, kind: Kind, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(lbl) = self.discrete.label(v) {
            write!(f, "{}", lbl)
        } else {
            let prefix = match kind {
                Kind::Bool => "b_",
                Kind::Int => "i_",
                Kind::Sym => "s_",
            };
            write!(f, "{}{}", prefix, usize::from(v))
        }
    }
}

impl Clone for Model {
    fn clone(&self) -> Self {
        Model {
            symbols: self.symbols.clone(),
            discrete: self.discrete.clone(),
            types: self.types.clone(),
            var_presence: self.var_presence.clone(),
            expressions: self.expressions.clone(),
            assignments: self.assignments.clone(),
            num_writers: self.num_writers,
        }
    }
}

/// Identifies an external writer to the model.
#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct WriterId(pub u8);
impl WriterId {
    pub fn new(num: impl Into<u8>) -> WriterId {
        WriterId(num.into())
    }

    pub fn cause(&self, cause: impl Into<u32>) -> Cause {
        Cause::inference(*self, cause)
    }
}

/// Provides write access to a model, making sure the built-in `WriterId` is always set.

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

impl Backtrack for Model {
    fn save_state(&mut self) -> DecLvl {
        self.discrete.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.discrete.num_saved()
    }

    fn restore_last(&mut self) {
        self.discrete.restore_last();
    }

    fn restore(&mut self, saved_id: DecLvl) {
        self.discrete.restore(saved_id);
    }
}

impl Assignment for Model {
    fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    fn entails(&self, literal: Bound) -> bool {
        self.discrete.entails(literal)
    }

    fn literal_of_expr(&self, expr: BExpr) -> Option<Bound> {
        match self.discrete.expr_binding.get(expr.expr) {
            Some(l) => {
                if expr.negated {
                    Some(!*l)
                } else {
                    Some(*l)
                }
            }
            None => None,
        }
    }

    fn var_domain(&self, var: impl Into<VarRef>) -> IntDomain {
        let (lb, ub) = self.discrete.domain_of(var.into());
        IntDomain { lb, ub }
    }

    fn to_owned_assignment(&self) -> SavedAssignment {
        SavedAssignment::from_model(self)
    }
}

/// Provides write access to a model for a particular module.
pub struct WModel<'a> {
    model: &'a mut Model,
    pub token: WriterId,
}

impl<'a> WModel<'a> {
    pub fn dup(&mut self) -> WModel<'_> {
        WModel {
            model: self.model,
            token: self.token,
        }
    }

    pub fn trail(&self) -> &ObsTrail<Event> {
        self.model.discrete.trail()
    }

    pub fn view(&self) -> &DiscreteModel {
        &self.model.discrete
    }

    #[deprecated]
    pub fn set(&mut self, lit: Bound, cause: impl Into<u32>) -> Result<bool, EmptyDomain> {
        let (var, rel, val) = lit.unpack();

        match rel {
            Relation::LEQ => self.set_upper_bound(var, val, cause),
            Relation::GT => self.set_lower_bound(var, val + 1, cause),
        }
    }
    pub fn set_upper_bound(
        &mut self,
        ivar: impl Into<VarRef>,
        ub: IntCst,
        cause: impl Into<u32>,
    ) -> Result<bool, EmptyDomain> {
        self.model.discrete.set_ub(ivar, ub, self.token.cause(cause))
    }
    pub fn set_lower_bound(
        &mut self,
        ivar: impl Into<VarRef>,
        lb: IntCst,
        cause: impl Into<u32>,
    ) -> Result<bool, EmptyDomain> {
        self.model.discrete.set_lb(ivar, lb, self.token.cause(cause))
    }
    pub fn bounds(&self, ivar: IVar) -> (IntCst, IntCst) {
        self.model.bounds(ivar)
    }
}

impl Assignment for WModel<'_> {
    fn symbols(&self) -> &SymbolTable {
        self.model.symbols()
    }

    fn entails(&self, literal: Bound) -> bool {
        self.model.entails(literal)
    }

    fn literal_of_expr(&self, expr: BExpr) -> Option<Bound> {
        self.model.literal_of_expr(expr)
    }

    fn var_domain(&self, var: impl Into<VarRef>) -> IntDomain {
        self.model.var_domain(var)
    }

    fn to_owned_assignment(&self) -> SavedAssignment {
        self.model.to_owned_assignment()
    }
}
