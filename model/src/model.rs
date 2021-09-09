use crate::bounds::Lit;
use crate::expressions::*;
use crate::extensions::{AssignmentExt, ExpressionFactoryExt, SavedAssignment, Shaped};
use crate::lang::*;
use crate::state::*;
use crate::symbols::SymbolTable;
use crate::types::TypeId;
use crate::Label;
use aries_backtrack::{Backtrack, DecLvl};
use aries_collections::ref_store::RefMap;
use std::sync::Arc;

/// Defines the structure of a model: variables names, types, relations, ...
#[derive(Clone)]
pub struct ModelShape {
    pub symbols: Arc<SymbolTable>,
    pub types: RefMap<VarRef, Type>,
    pub expressions: Expressions,
    pub labels: RefMap<VarRef, String>,
    num_writers: u8,
}

impl ModelShape {
    pub fn new() -> Self {
        Self::new_with_symbols(Arc::new(SymbolTable::empty()))
    }

    pub fn new_with_symbols(symbols: Arc<SymbolTable>) -> Self {
        let mut m = ModelShape {
            symbols,
            types: Default::default(),
            expressions: Default::default(),
            labels: Default::default(),
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

    pub fn intern_boolean_expression(&mut self, e: Expr) -> BExpr {
        let handle = self.expressions.intern(e);
        BExpr {
            expr: handle,
            negated: false,
        }
    }

    // ======= Expression reification =====

    pub fn interned_expr(&self, handle: ExprHandle) -> Option<Lit> {
        self.expressions.as_lit(handle)
    }
}

impl Default for ModelShape {
    fn default() -> Self {
        Self::new()
    }
}

///
#[derive(Clone)]
pub struct Model {
    /// Structure of the model and metadata of its various components.
    pub shape: ModelShape,
    /// Domain of all variables, defining the current state of the Model.
    pub state: Domains,
}

impl Model {
    pub fn new() -> Self {
        Self::new_with_symbols(Arc::new(SymbolTable::empty()))
    }

    pub fn new_with_symbols(symbols: Arc<SymbolTable>) -> Self {
        Model {
            shape: ModelShape::new_with_symbols(symbols),
            state: Domains::new(),
        }
    }

    pub fn new_write_token(&mut self) -> WriterId {
        self.shape.new_write_token()
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
        self.shape.set_label(var, label);
        self.shape.set_type(var, Type::Bool);
        BVar::new(var)
    }

    fn create_bvar(&mut self, presence: Option<Lit>, label: impl Into<Label>) -> BVar {
        let dvar = if let Some(presence) = presence {
            self.state.new_optional_var(0, 1, presence)
        } else {
            self.state.new_var(0, 1)
        };
        self.shape.set_label(dvar, label);
        self.shape.set_type(dvar, Type::Bool);
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
        self.shape.set_label(dvar, label);
        self.shape.set_type(dvar, Type::Int);
        IVar::new(dvar)
    }

    pub fn new_sym_var(&mut self, tpe: TypeId, label: impl Into<Label>) -> SVar {
        self.create_sym_var(tpe, None, label)
    }

    pub fn new_optional_sym_var(&mut self, tpe: TypeId, presence: impl Into<Lit>, label: impl Into<Label>) -> SVar {
        self.create_sym_var(tpe, Some(presence.into()), label)
    }

    fn create_sym_var(&mut self, tpe: TypeId, presence: Option<Lit>, label: impl Into<Label>) -> SVar {
        let instances = self.shape.symbols.instances_of_type(tpe);
        if let Some((lb, ub)) = instances.bounds() {
            let lb = usize::from(lb) as IntCst;
            let ub = usize::from(ub) as IntCst;
            let dvar = if let Some(presence) = presence {
                self.state.new_optional_var(lb, ub, presence)
            } else {
                self.state.new_var(lb, ub)
            };
            self.shape.set_label(dvar, label);
            self.shape.set_type(dvar, Type::Sym(tpe));
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

    pub fn create_binding(&mut self, expr: ExprHandle) -> Lit {
        if let Some(lit) = self.shape.interned_expr(expr) {
            lit
        } else {
            let name = format!("{}", self.fmt(BAtom::Expr(BExpr { expr, negated: false })));
            let var = self.new_bvar(name);
            let lit = var.true_lit();
            self.record_binding(expr, lit);
            lit
        }
    }

    pub fn record_binding(&mut self, handle: ExprHandle, literal: Lit) {
        assert!(self.shape.expressions.as_lit(handle).is_none());
        self.shape.expressions.bind(handle, literal);
    }

    pub fn reify(&mut self, b: BAtom) -> Lit {
        match b {
            BAtom::Cst(true) => Lit::TRUE,
            BAtom::Cst(false) => Lit::FALSE,
            BAtom::Literal(b) => b,
            BAtom::Expr(e) => {
                let BExpr { expr: handle, negated } = e;
                let lit = self.create_binding(handle);
                if negated {
                    !lit
                } else {
                    lit
                }
            }
        }
    }

    // =========== Formatting ==============

    pub fn fmt(&self, atom: impl Into<Atom>) -> impl std::fmt::Display + '_ {
        let atom = atom.into();
        crate::extensions::fmt(atom, self)
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
        self.shape.intern_boolean_expression(expr)
    }

    fn presence_literal(&self, variable: VarRef) -> Lit {
        self.state.presence(variable)
    }
}

impl AssignmentExt for Model {
    fn symbols(&self) -> &SymbolTable {
        &self.shape.symbols
    }

    fn entails(&self, literal: Lit) -> bool {
        self.state.entails(literal)
    }

    fn literal_of_expr(&self, expr: BExpr) -> Option<Lit> {
        match self.shape.expressions.as_lit(expr.expr) {
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

impl Shaped for Model {
    fn get_shape(&self) -> &ModelShape {
        &self.shape
    }
}
