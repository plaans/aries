use std::convert::TryFrom;
use std::fmt::Formatter;
use std::sync::Arc;

use crate::backtrack::{Backtrack, DecLvl};
use crate::collections::ref_store::RefMap;
use crate::core::literals::StableLitSet;
use crate::core::state::*;
use crate::core::*;
use crate::model::extensions::{AssignmentExt, SavedAssignment, Shaped};
use crate::model::label::{Label, VariableLabels};
use crate::model::lang::expr::or;
use crate::model::lang::reification::Reification;
use crate::model::lang::*;
use crate::model::model_impl::scopes::Scopes;
use crate::model::symbols::SymbolTable;
use crate::model::types::TypeId;
use crate::reif::{ReifExpr, Reifiable};

mod scopes;

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Constraint {
    /// Constraint enforcing that the left and right terms evaluate to the same value.
    Reified(ReifExpr, Lit),
}

impl std::fmt::Display for Constraint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Constraint::Reified(r, l) => {
                write!(f, "{l:?} <=> {r}")
            }
        }
    }
}

/// Defines the structure of a model: variables names, types, relations, ...
#[derive(Clone)]
pub struct ModelShape<Lbl> {
    pub symbols: Arc<SymbolTable>,
    pub types: RefMap<VarRef, Type>,
    pub expressions: Reification,
    pub constraints: Vec<Constraint>,
    pub labels: VariableLabels<Lbl>,
    pub conjunctive_scopes: Scopes,
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
            constraints: Default::default(),
            labels: Default::default(),
            conjunctive_scopes: Default::default(),
        }
    }

    fn set_label(&mut self, var: VarRef, l: impl Into<Lbl>) {
        self.labels.insert(var, l.into())
    }
    pub fn get_variable(&self, label: &Lbl) -> Option<VarRef> {
        match *self.labels.variables_with_label(label) {
            [] => None,
            [var] => Some(var),
            _ => panic!("More than one variable with label: {label:?}"),
        }
    }
    fn set_type(&mut self, var: VarRef, typ: Type) {
        self.types.insert(var, typ);
    }

    fn add_reification_constraint(&mut self, value: Lit, expr: ReifExpr) {
        let c = Constraint::Reified(expr, value);
        tracing::trace!("Adding constraint: {}", c);
        self.constraints.push(c)
    }

    /// Given a TOTAL assignment, check that the all constraints are satisfied.
    /// NOTE: Currently not really polished and intended for internal use.
    pub(crate) fn validate(&self, assignment: &Domains) -> anyhow::Result<()> {
        for c in &self.constraints {
            let Constraint::Reified(expr, reified) = c;
            if assignment.present(reified.variable()).unwrap() {
                let actual_value = expr.eval(assignment);
                let expected_value = Some(assignment.value(*reified).unwrap());
                anyhow::ensure!(
                    actual_value == expected_value,
                    "{}: {:?}  !=  {:?} [{:?}]",
                    expr,
                    actual_value,
                    expected_value,
                    reified
                );
            } else {
                // Underspecified: we may be able to determine a value on the
                // expression side (e.g. with short-circuiting "or") even though we are not in the
                // validity scope of the literal.
            }
        }
        Ok(())
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

    pub fn with_domains(mut self, domains: Domains) -> Model<Lbl> {
        self.state = domains;
        self
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

    /// Returns a presence literal that is true iff all given presence literal are true.
    pub fn get_conjunctive_scope(&mut self, presence_variables: &[Lit]) -> Lit {
        assert!(presence_variables
            .iter()
            .all(|l| self.state.presence(l.variable()) == Lit::TRUE));
        let empty: &[Lit] = &[];
        let scope = ValidityScope::new(presence_variables.iter().copied(), empty.iter().copied());
        let scope = scope.to_conjunction(
            |l| self.shape.conjunctive_scopes.conjuncts(l),
            |l| self.state.entails(l) && self.state.entailing_level(l) == DecLvl::ROOT,
        );
        self.new_conjunctive_presence_variable(scope)
    }

    /// Returns a literal whose presence is `scope` and that is always true.
    ///
    /// THis is functionnaly equivalent to creating a new optional boolean variable
    /// with domain `[1,1]` with `presence=scope` but will ensure that only one such
    /// variable is created in this scope.
    pub fn get_tautology_of_scope(&mut self, scope: Lit) -> Lit {
        self.shape
            .conjunctive_scopes
            .get_tautology_of_scope(scope)
            .unwrap_or_else(|| {
                let var = self.state.new_optional_var(1, 1, scope);
                let lit = var.geq(1);
                self.shape.set_type(var, Type::Bool);
                self.shape.conjunctive_scopes.set_tautology_of_scope(scope, lit);
                lit
            })
    }

    fn new_conjunctive_presence_variable(&mut self, set: impl Into<StableLitSet>) -> Lit {
        let set = set.into();
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
                // we create a new literal that is always false
                // NOTE: we cannot use `Lit::FALSE` directly because we need to uniquely identify
                //       the literal as the conjunction of the other two in some corner cases.
                let l = self.state.new_var(0, 0).geq(1);
                self.shape.set_type(l.variable(), Type::Bool);
                Some(l)
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
            self.enforce(or(clause), []);
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

    pub fn new_fvar(&mut self, num_lb: IntCst, num_ub: IntCst, denom: IntCst, label: impl Into<Lbl>) -> FVar {
        let ivar = self.new_ivar(num_lb, num_ub, label);
        FVar::new(ivar, denom)
    }
    pub fn new_optional_fvar(
        &mut self,
        num_lb: IntCst,
        num_ub: IntCst,
        denom: IntCst,
        presence: Lit,
        label: impl Into<Lbl>,
    ) -> FVar {
        let ivar = self.new_optional_ivar(num_lb, num_ub, presence, label);
        FVar::new(ivar, denom)
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
        self.shape.set_type(dvar, Type::Int { lb, ub });
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
            // no instances for this type, we should create a variable with an empty domain which is only allowed for optional variable
            println!("WARNING: workaround for empty domain of optional vars (Github Issue #28)");
            let p = if let Some(presence) = presence {
                // this is an optional variable, force it to be absent
                self.state
                    .set(!presence, Cause::Decision) // TODO: fix decision cause
                    .expect("An optional but necessarily present variable has an empty integer domain.");
                // create a a variable with arbitrary domain (which will never be used as is forced to be absent)
                self.state.new_optional_var(0, 0, presence)
            } else {
                // non-optional variable, break on assumption of non-empty domain in the model
                panic!("Variable with empty symbolic domain.");
            };
            SVar::new(p, tpe)
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
    /// The returned literal is *optional* and defined such that it is
    /// present iff the expression is valid (typically meaning that all
    /// variables involved in the expression are present).
    ///
    /// If the expression was already interned, the handle to the previously inserted
    /// instance will be returned.
    pub fn reify<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Lit {
        let decomposed = expr.decompose(self);
        self.reify_core(decomposed, false)
    }

    fn simplify(&self, expr: &mut ReifExpr) {
        let entailed = |lit| {
            if self.state.current_decision_level() == DecLvl::ROOT {
                self.entails(lit)
            } else {
                lit == Lit::TRUE
            }
        };
        let negated = |lit: Lit| entailed(!lit);
        match expr {
            ReifExpr::Or(disjuncts) if disjuncts.iter().any(|&l| entailed(l)) => *expr = ReifExpr::Lit(Lit::TRUE),
            ReifExpr::Or(disjuncts) => {
                disjuncts.retain(|&l| !negated(l));
                match disjuncts.len() {
                    0 => *expr = ReifExpr::Lit(Lit::FALSE),
                    1 => *expr = ReifExpr::Lit(disjuncts[0]),
                    _ => {}
                }
            }
            ReifExpr::And(conjuncts) if conjuncts.iter().any(|&l| negated(l)) => *expr = ReifExpr::Lit(Lit::FALSE),
            ReifExpr::And(conjuncts) => {
                conjuncts.retain(|l| !entailed(*l));
                match conjuncts.len() {
                    0 => *expr = ReifExpr::Lit(Lit::TRUE),
                    1 => *expr = ReifExpr::Lit(conjuncts[0]),
                    _ => {}
                }
            }
            ReifExpr::Linear(lin) => *lin = lin.simplify(),
            ReifExpr::Eq(v1, v2) => {
                if v1 < v2 {
                    std::mem::swap(v1, v2);
                }
                let (lb1, ub1) = self.state.bounds(*v1);
                let (lb2, ub2) = self.state.bounds(*v2);
                if ub1 < lb2 || ub2 < lb1 {
                    *expr = ReifExpr::Lit(Lit::FALSE);
                } else if lb1 == ub1 && ub1 == lb2 && lb2 == ub2 {
                    *expr = ReifExpr::Lit(Lit::TRUE);
                }
            }
            ReifExpr::EqVal(v1, v2) => {
                let (lb, ub) = self.state.bounds(*v1);
                if *v2 < lb || *v2 > ub {
                    *expr = ReifExpr::Lit(Lit::FALSE)
                } else if *v2 == lb && *v2 == ub {
                    *expr = ReifExpr::Lit(Lit::TRUE)
                }
            }
            _ => {}
        }
    }

    /// Reify the given expression.
    /// If `use_tautology` is true, then the tautology of the scope will be used (meaning that the expression will
    /// be constrained to always evaluate to true!).
    pub(crate) fn reify_core(&mut self, mut expr: ReifExpr, use_tautology: bool) -> Lit {
        self.simplify(&mut expr);

        if let Some(l) = self.shape.expressions.interned(&expr) {
            l
        } else {
            let scope = expr.scope(|var| self.state.presence(var));
            let scope = scope.to_conjunction(
                |l| self.shape.conjunctive_scopes.conjuncts(l),
                |l| self.state.entails(l),
            );
            let scope = self.new_conjunctive_presence_variable(scope);
            let lit = if use_tautology {
                self.get_tautology_of_scope(scope)
            } else {
                let var = self.state.new_optional_var(0, 1, scope);
                self.shape.set_type(var, Type::Bool);
                var.geq(1)
            };
            self.shape.expressions.intern_as(expr.clone(), lit);
            self.shape.add_reification_constraint(lit, expr);

            lit
        }
    }

    pub fn check_reified<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Option<Lit> {
        let decomposed = &mut expr.decompose(self);
        self.simplify(decomposed);
        self.shape.expressions.interned(decomposed)
    }

    /// Enforce the given expression to be true whenever all literals of the scope are true.
    /// Similar to posting a constraint in CP solvers.
    ///
    /// Internally, the expression is reified to an optional literal that is true, when the expression
    /// is valid and absent otherwise.
    pub fn enforce<Expr: Reifiable<Lbl>>(&mut self, expr: Expr, scope: impl IntoIterator<Item = Lit>) {
        debug_assert_eq!(self.state.current_decision_level(), DecLvl::ROOT);
        let mut expr = expr.decompose(self);
        self.simplify(&mut expr);

        let scope = self.new_conjunctive_presence_variable(scope);
        debug_assert!(
            {
                // compute the scope in which the expression is valid
                let expr_scope = expr.scope(|var| self.state.presence(var));
                let expr_scope = expr_scope.to_conjunction(
                    |l| self.shape.conjunctive_scopes.conjuncts(l),
                    |l| self.state.entails(l),
                );
                let expr_scope = self.new_conjunctive_presence_variable(expr_scope);
                self.state.implies(scope, expr_scope)
            },
            "Error in scope definition: the expression {expr:?} is not always define in the provided scope."
        );

        // retrieve or create an optional variable that is always true in the scope
        let tauto = self.get_tautology_of_scope(scope);

        self.bind(expr, tauto);
    }

    pub fn enforce_all<Expr: Reifiable<Lbl>>(
        &mut self,
        bools: impl IntoIterator<Item = Expr>,
        scope: impl IntoIterator<Item = Lit> + Clone,
    ) {
        for b in bools {
            self.enforce(b, scope.clone());
        }
    }

    /// Record that `b <=> literal`
    pub fn bind<Expr: Reifiable<Lbl>>(&mut self, expr: Expr, value: Lit) {
        let mut expr = expr.decompose(self);
        self.simplify(&mut expr);

        // compute the validity scope of the expression, which be larger than the one of the value
        let expression_scope = expr.scope(|var| self.state.presence(var));
        let expression_scope = expression_scope.to_conjunction(
            |l| self.shape.conjunctive_scopes.conjuncts(l),
            |l| self.state.entails(l),
        );
        let expression_scope = self.new_conjunctive_presence_variable(expression_scope);
        debug_assert!(
            self.state
                .implies(self.presence_literal(value.variable()), expression_scope),
            "Inconsistent validity scope between the expression and the literal. {expr:?} <=> {value:?}"
        );

        if let Some(reified) = self.shape.expressions.interned(&expr) {
            // expression already reified, unify it with expected value
            self.bind_literals(value, reified)
        } else if expression_scope == self.presence_literal(value.variable()) {
            // not yet reified and compatible scopes, propose our literal as the reification
            self.shape.expressions.intern_as(expr.clone(), value);
            self.shape.add_reification_constraint(value, expr);
        } else {
            // not yet reified but our literal cannot be used directly because it has a different scope
            // if the literal is already true for a linear constraint, use the tautology of the expression scope as reification
            // this is done because we do not handle reified linear constraint for the moment
            let use_tautology = self.entails(value) && matches!(expr, ReifExpr::Linear(_));
            let reified = self.reify_core(expr, use_tautology);
            self.bind_literals(value, reified);
        }
    }

    /// Record that `b <=> literal`
    fn bind_literals(&mut self, l1: Lit, l2: Lit) {
        if l1 != l2 {
            self.shape.add_reification_constraint(l1, ReifExpr::Lit(l2))
        }
    }

    // =========== Formatting ==============

    pub fn fmt(&self, atom: impl Into<Atom>) -> impl std::fmt::Display + '_ {
        let atom = atom.into();
        crate::model::extensions::fmt(atom, self)
    }

    pub fn print_state(&self) {
        for v in self.state.variables() {
            let prez = format!("[{:?}]", self.presence_literal(v));
            let v_str = format!("{v:?}");
            print!("{prez:<6}  {v_str:<6} <- {:?}", self.state.domain(v));
            if let Some(lbl) = self.get_label(v) {
                println!("    {lbl:?}");
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

    fn presence_literal(&self, variable: impl Term) -> Lit {
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
