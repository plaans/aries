use std::convert::TryFrom;
use std::sync::Arc;

use aries_backtrack::{Backtrack, DecLvl};
use aries_collections::ref_store::RefMap;
use aries_core::WriterId;

use crate::extensions::{AssignmentExt, SavedAssignment, Shaped};
use crate::label::{Label, VariableLabels};
use crate::lang::expr::{or, Normalize};
use crate::lang::reification::{ReifiableExpr, Reification};
use crate::lang::*;
use crate::model::scopes::Scopes;
use crate::symbols::SymbolTable;
use crate::types::TypeId;
use aries_core::literals::StableLitSet;
use aries_core::state::*;
use aries_core::*;

mod scopes;

/// Defines the structure of a model: variables names, types, relations, ...
#[derive(Clone)]
pub struct ModelShape<Lbl> {
    pub symbols: Arc<SymbolTable>,
    pub types: RefMap<VarRef, Type>,
    pub expressions: Reification,
    pub labels: VariableLabels<Lbl>,
    pub conjunctive_scopes: Scopes,
    num_writers: u8,
}

impl<Lbl: Label> ModelShape<Lbl> {
    pub fn new() -> Self {
        Self::new_with_symbols(Arc::new(SymbolTable::empty()))
    }

    pub fn new_with_symbols(symbols: Arc<SymbolTable>) -> Self {
        ModelShape {
            symbols,
            types: Default::default(),
            expressions: Default::default(),
            labels: Default::default(),
            conjunctive_scopes: Default::default(),
            num_writers: 0,
        }
    }

    pub fn new_write_token(&mut self) -> WriterId {
        self.num_writers += 1;
        WriterId(self.num_writers - 1)
    }

    fn set_label(&mut self, var: VarRef, l: impl Into<Lbl>) {
        self.labels.insert(var, l.into())
    }
    pub fn get_variable(&self, label: &Lbl) -> Option<VarRef> {
        match *self.labels.variables_with_label(label) {
            [] => None,
            [var] => Some(var),
            _ => panic!("More than one variable with label: {:?}", label),
        }
    }
    fn set_type(&mut self, var: VarRef, typ: Type) {
        self.types.insert(var, typ);
    }
}

impl<Lbl: Label> Default for ModelShape<Lbl> {
    fn default() -> Self {
        Self::new()
    }
}

/// Description problem, composed of its shape (variable declaration, composed
/// expressions, ...) and its state (the currently admissible values for each variable).
#[derive(Clone)]
pub struct Model<Lbl> {
    /// Structure of the model and metadata of its various components.
    pub shape: ModelShape<Lbl>,
    /// Domain of all variables, defining the current state of the Model.
    pub state: Domains,
}

impl<Lbl: Label> Model<Lbl> {
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

    pub fn new_bvar(&mut self, label: impl Into<Lbl>) -> BVar {
        self.create_bvar(None, label)
    }

    pub fn new_optional_bvar(&mut self, presence: Lit, label: impl Into<Lbl>) -> BVar {
        self.create_bvar(Some(presence), label)
    }

    pub fn new_presence_variable(&mut self, scope: Lit, label: impl Into<Lbl>) -> BVar {
        let lit = self.state.new_var(0, 1).geq(1);
        self.shape.conjunctive_scopes.insert(StableLitSet::from([lit]), lit);
        self.state.add_implication(lit, scope);
        let var = lit.variable();
        self.shape.set_label(var, label);
        self.shape.set_type(var, Type::Bool);
        BVar::new(var)
    }

    fn new_conjunctive_presence_variable(&mut self, set: StableLitSet) -> Lit {
        if let Some(l) = self.shape.conjunctive_scopes.get(&set) {
            // scope already exists, return it immediately
            return l;
        }

        // let the scope's set be composed of { v1, v2, ..., vn }
        // we need to create a new literal `l` such that  `l <=> v1 & v2 & ... & vn`

        // first, try to avoid creating a new literal, by simplifying the conjunctive set
        let attempt: Option<Lit> = if let Ok([v1]) = <[Lit; 1]>::try_from(&set) {
            // single literal v1, let l = v1
            Some(v1)
        } else if let Ok([v1, v2]) = <[Lit; 2]>::try_from(&set) {
            // only two literals v1 and v2
            if self.state.implies(v1, v2) {
                // v1 => v2, let l = v1
                Some(v1)
            } else if self.state.implies(v2, v1) {
                // v2 => v1, let l = v2
                Some(v2)
            } else if self.state.exclusive(v1, v2) {
                // `v1 & v2` is always false, let l = FALSE
                Some(Lit::FALSE)
            } else {
                None // no simplification found, proceed
            }
        } else {
            None // no simplification found, proceed
        };

        let l = attempt.unwrap_or_else(|| {
            // Simplification did not succeed.
            // create a new literal `l` such that `l <=> (v1 & v2 & ... & vn)`, decomposed to:
            // - `l => v1`, `l => v2`, ...
            // - `l | !v1 | !v2 | ... | !vn`
            let l = self.state.new_var(0, 1).geq(1);
            self.shape.set_type(l.variable(), Type::Bool);
            let mut clause = vec![l];
            for v_i in set.literals() {
                self.state.add_implication(l, v_i);
                clause.push(!v_i);
            }
            self.enforce(or(clause));
            l
        });
        self.shape.conjunctive_scopes.insert(set, l);
        l
    }

    fn create_bvar(&mut self, presence: Option<Lit>, label: impl Into<Lbl>) -> BVar {
        let dvar = if let Some(presence) = presence {
            self.state.new_optional_var(0, 1, presence)
        } else {
            self.state.new_var(0, 1)
        };
        self.shape.set_label(dvar, label);
        self.shape.set_type(dvar, Type::Bool);
        BVar::new(dvar)
    }

    pub fn new_ivar(&mut self, lb: IntCst, ub: IntCst, label: impl Into<Lbl>) -> IVar {
        self.create_ivar(lb, ub, None, label)
    }

    pub fn new_optional_ivar(&mut self, lb: IntCst, ub: IntCst, presence: Lit, label: impl Into<Lbl>) -> IVar {
        self.create_ivar(lb, ub, Some(presence), label)
    }

    fn create_ivar(&mut self, lb: IntCst, ub: IntCst, presence: Option<Lit>, label: impl Into<Lbl>) -> IVar {
        let dvar = if let Some(presence) = presence {
            self.state.new_optional_var(lb, ub, presence)
        } else {
            self.state.new_var(lb, ub)
        };
        self.shape.set_label(dvar, label);
        self.shape.set_type(dvar, Type::Int);
        IVar::new(dvar)
    }

    pub fn new_sym_var(&mut self, tpe: TypeId, label: impl Into<Lbl>) -> SVar {
        self.create_sym_var(tpe, None, label)
    }

    pub fn new_optional_sym_var(&mut self, tpe: TypeId, presence: impl Into<Lit>, label: impl Into<Lbl>) -> SVar {
        self.create_sym_var(tpe, Some(presence.into()), label)
    }

    fn create_sym_var(&mut self, tpe: TypeId, presence: Option<Lit>, label: impl Into<Lbl>) -> SVar {
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

    /// Interns the given expression and returns an equivalent literal.
    /// If the expression was already interned, the handle to the previously inserted
    /// instance will be returned.
    pub fn reify<T: ReifiableExpr, Expr: Normalize<T>>(&mut self, expr: Expr) -> Lit {
        let e1 = expr.normalize();
        let e2 = expr.normalize(); // TODO: avoid this duplicated work (requires update to interned)
        if let Some(l) = self.shape.expressions.interned(e1) {
            l
        } else {
            let scope = e2.validity_scope(&|var| self.state.presence(var));
            let scope = scope.to_conjunction(
                |l| self.shape.conjunctive_scopes.conjuncts(l),
                |l| self.state.entails(l),
            );
            let scope = self.new_conjunctive_presence_variable(scope);
            let var = self.state.new_optional_var(0, 1, scope);
            let lit = var.geq(1);
            self.shape.set_type(var, Type::Bool);
            self.shape.expressions.intern_as(e2, lit);
            lit
        }
    }

    pub fn enforce<Expr: Normalize<T>, T: ReifiableExpr>(&mut self, b: Expr) {
        // TODO: bind to optional
        self.shape.expressions.bind(b, Lit::TRUE)
    }

    pub fn enforce_all<Expr: Normalize<T>, T: ReifiableExpr>(&mut self, bools: impl IntoIterator<Item = Expr>) {
        for b in bools {
            self.enforce(b);
        }
    }

    /// Record that `b <=> literal`
    pub fn bind<Expr: Normalize<T>, T: ReifiableExpr>(&mut self, expr: Expr, literal: Lit) {
        self.shape.expressions.bind(expr, literal);
    }

    /// Record that `b <=> literal`
    pub fn bind_literals(&mut self, l1: Lit, l2: Lit) {
        self.shape.expressions.bind_literals(l1, l2);
    }

    // =========== Formatting ==============

    pub fn fmt(&self, atom: impl Into<Atom>) -> impl std::fmt::Display + '_ {
        let atom = atom.into();
        crate::extensions::fmt(atom, self)
    }

    pub fn print_state(&self) {
        for v in self.state.variables() {
            print!("{:?} <- {:?}", v, self.state.domain(v));
            if let Some(lbl) = self.get_label(v) {
                println!("    {:?}", lbl);
            } else {
                println!()
            }
        }
    }
}

impl<Lbl: Label> Default for Model<Lbl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Lbl> Backtrack for Model<Lbl> {
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

impl<Lbl> AssignmentExt for Model<Lbl> {
    fn entails(&self, literal: Lit) -> bool {
        self.state.entails(literal)
    }

    fn var_domain(&self, var: impl Into<IAtom>) -> IntDomain {
        self.state.var_domain(var)
    }

    fn presence_literal(&self, variable: VarRef) -> Lit {
        self.state.presence(variable)
    }

    fn to_owned_assignment(&self) -> SavedAssignment {
        self.state.clone()
    }
}

impl<Lbl: Label> Shaped<Lbl> for Model<Lbl> {
    fn get_shape(&self) -> &ModelShape<Lbl> {
        &self.shape
    }
}
