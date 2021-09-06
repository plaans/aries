use crate::bounds::Lit;
use crate::expressions::*;
use crate::extensions::{AssignmentExt, ExpressionFactoryExt, SavedAssignment};
use crate::lang::*;
use crate::state::*;
use crate::symbols::SymbolTable;
use crate::types::TypeId;
use crate::Label;
use aries_backtrack::{Backtrack, DecLvl};
use aries_collections::ref_store::RefMap;
use aries_utils::Fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct Model {
    pub symbols: Arc<SymbolTable>,
    pub state: OptDomains,
    pub types: RefMap<VarRef, Type>,
    pub expressions: Expressions,
    labels: RefMap<VarRef, String>,
    assignments: Vec<SavedAssignment>,
    num_writers: u8,
}

impl Model {
    pub fn new() -> Self {
        Self::new_with_symbols(Arc::new(SymbolTable::empty()))
    }

    pub fn new_with_symbols(symbols: Arc<SymbolTable>) -> Self {
        let mut m = Model {
            symbols,
            state: OptDomains::new(),
            types: Default::default(),
            expressions: Default::default(),
            labels: Default::default(),
            assignments: vec![],
            num_writers: 0,
        };
        m.set_label(VarRef::ZERO, "ZERO");
        m
    }

    pub fn new_write_token(&mut self) -> WriterId {
        self.num_writers += 1;
        WriterId(self.num_writers - 1)
    }

    fn set_label(&mut self, var: VarRef, l: impl Into<Label>) {
        if let Some(str) = l.into().lbl {
            self.labels.insert(var, str)
        }
    }
    fn set_type(&mut self, var: VarRef, typ: Type) {
        self.types.insert(var, typ);
    }

    pub fn new_bvar<L: Into<Label>>(&mut self, label: L) -> BVar {
        self.create_bvar(None, label)
    }

    pub fn new_optional_bvar<L: Into<Label>>(&mut self, presence: Lit, label: L) -> BVar {
        self.create_bvar(Some(presence), label)
    }

    pub fn new_presence_variable(&mut self, scope: Lit, label: impl Into<Label>) -> BVar {
        let lit = self.state.new_presence_literal(scope);
        let var = lit.variable();
        self.set_label(var, label);
        self.set_type(var, Type::Bool);
        BVar::new(var)
    }

    fn create_bvar(&mut self, presence: Option<Lit>, label: impl Into<Label>) -> BVar {
        let dvar = if let Some(presence) = presence {
            self.state.new_optional_var(0, 1, presence)
        } else {
            self.state.new_var(0, 1)
        };
        self.set_label(dvar, label);
        self.set_type(dvar, Type::Bool);
        BVar::new(dvar)
    }

    pub fn new_ivar(&mut self, lb: IntCst, ub: IntCst, label: impl Into<Label>) -> IVar {
        self.create_ivar(lb, ub, None, label)
    }

    pub fn new_optional_ivar(&mut self, lb: IntCst, ub: IntCst, presence: Lit, label: impl Into<Label>) -> IVar {
        self.create_ivar(lb, ub, Some(presence), label)
    }

    fn create_ivar(&mut self, lb: IntCst, ub: IntCst, presence: Option<Lit>, label: impl Into<Label>) -> IVar {
        let dvar = if let Some(presence) = presence {
            self.state.new_optional_var(lb, ub, presence)
        } else {
            self.state.new_var(lb, ub)
        };
        self.set_label(dvar, label);
        self.set_type(dvar, Type::Int);
        IVar::new(dvar)
    }

    pub fn new_sym_var(&mut self, tpe: TypeId, label: impl Into<Label>) -> SVar {
        self.create_sym_var(tpe, None, label)
    }

    pub fn new_optional_sym_var(&mut self, tpe: TypeId, presence: impl Into<Lit>, label: impl Into<Label>) -> SVar {
        self.create_sym_var(tpe, Some(presence.into()), label)
    }

    fn create_sym_var(&mut self, tpe: TypeId, presence: Option<Lit>, label: impl Into<Label>) -> SVar {
        let instances = self.symbols.instances_of_type(tpe);
        if let Some((lb, ub)) = instances.bounds() {
            let lb = usize::from(lb) as IntCst;
            let ub = usize::from(ub) as IntCst;
            let dvar = if let Some(presence) = presence {
                self.state.new_optional_var(lb, ub, presence)
            } else {
                self.state.new_var(lb, ub)
            };
            self.set_label(dvar, label);
            self.set_type(dvar, Type::Sym(tpe));
            SVar::new(dvar, tpe)
        } else {
            // no instances for this type, make a variable with empty domain
            //self.discrete.new_var(1, 0, label)
            panic!(
                "Variable with empty symbolic domain (note that we do not properly handle optionality in this case)"
            );
        }
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

    pub fn bounds(&self, ivar: impl Into<VarRef>) -> (IntCst, IntCst) {
        self.state.bounds(ivar.into())
    }

    pub fn intern_boolean_expression(&mut self, e: Expr) -> BExpr {
        let handle = self.expressions.intern(e);
        BExpr {
            expr: handle,
            negated: false,
        }
    }

    // ================= Assignments =========================

    pub fn current_assignment(&self) -> &impl AssignmentExt {
        self
    }

    pub fn save_current_assignment(&mut self, overwrite_previous: bool) {
        let ass = SavedAssignment::from_model(self);
        if overwrite_previous {
            self.assignments.pop();
        }
        self.assignments.push(ass);
    }

    pub fn last_saved_assignment(&self) -> Option<&impl AssignmentExt> {
        self.assignments.last()
    }

    // ======= Expression reification =====

    pub fn interned_expr(&self, handle: ExprHandle) -> Option<Lit> {
        self.expressions.as_lit(handle)
    }

    pub fn intern_expr(&mut self, expr: ExprHandle) -> Lit {
        if let Some(lit) = self.interned_expr(expr) {
            lit
        } else {
            let name = format!("{}", self.fmt(BAtom::Expr(BExpr { expr, negated: false })));
            let var = self.new_bvar(name);
            let lit = var.true_lit();
            self.bind_expr(expr, lit);
            lit
        }
    }

    pub fn bind_expr(&mut self, handle: ExprHandle, literal: Lit) {
        assert!(self.expressions.as_lit(handle).is_none());
        self.expressions.bind(handle, literal);
    }

    pub fn reify(&mut self, b: BAtom) -> Lit {
        match b {
            BAtom::Cst(true) => Lit::TRUE,
            BAtom::Cst(false) => Lit::FALSE,
            BAtom::Literal(b) => b,
            BAtom::Expr(e) => {
                let BExpr { expr: handle, negated } = e;
                let lit = self.intern_expr(handle);
                if negated {
                    !lit
                } else {
                    lit
                }
            }
        }
    }

    // =========== Formatting ==============

    pub fn label(&self, var: impl Into<VarRef>) -> Option<&str> {
        self.labels.get(var.into()).map(|s| s.as_str())
    }

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
            BAtom::Literal(b) => {
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
        if let Some(lbl) = self.label(v) {
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
        self.state.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.state.num_saved()
    }

    fn restore_last(&mut self) {
        self.state.restore_last();
    }

    fn restore(&mut self, saved_id: DecLvl) {
        self.state.restore(saved_id);
    }
}

impl ExpressionFactoryExt for Model {
    fn intern_bool(&mut self, expr: Expr) -> BExpr {
        self.intern_boolean_expression(expr)
    }

    fn presence_literal(&self, variable: VarRef) -> Lit {
        self.state.presence(variable)
    }
}

impl AssignmentExt for Model {
    fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    fn entails(&self, literal: Lit) -> bool {
        self.state.entails(literal)
    }

    fn literal_of_expr(&self, expr: BExpr) -> Option<Lit> {
        match self.expressions.as_lit(expr.expr) {
            Some(l) => {
                if expr.negated {
                    Some(!l)
                } else {
                    Some(l)
                }
            }
            None => None,
        }
    }

    fn var_domain(&self, var: impl Into<VarRef>) -> IntDomain {
        let (lb, ub) = self.state.bounds(var.into());
        IntDomain { lb, ub }
    }

    fn presence_literal(&self, variable: VarRef) -> Lit {
        self.state.presence(variable)
    }

    fn to_owned_assignment(&self) -> SavedAssignment {
        SavedAssignment::from_model(self)
    }
}
