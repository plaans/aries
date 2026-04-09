use crate::{
    core::{state::Evaluable, views::Dom},
    model::Label,
    prelude::*,
    reif::ReifExpr,
};

/// Trait that abstracts the core capabilities of a mutable [`Model`] and used as backend for posting constraints
/// to the model.
///
/// TODO: the name `Store` is mostly historical and should be change to align it with other views.
pub trait Store: Dom {
    fn new_literal(&mut self, presence: Lit) -> Lit;
    fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> VarRef;
    fn get_implicant(&mut self, e: ReifExpr) -> Lit;
    fn add_implies(&mut self, l: Lit, e: ReifExpr);

    fn conjunctive_scope(&mut self, lits: &[Lit]) -> Lit;
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit;

    /// Adds an assertion on solutions, i.e., an expression that is assumed to always evaluate to true.
    ///
    /// At this point, this is a place holder to allow specifying such well-formedness assumptions.
    /// The objective would be to have an explicit check at the end.
    fn add_assertion(&mut self, _condition: impl Evaluable<Value = bool>) {
        // TODO: post a constraint that never propagates but is checked in the solution
    }
}

/// Convenience trait for anything that wraps a [`Model`]. Implementing [`ModelWrapper`] will automatically derive
/// [`Store`].
pub trait ModelWrapper {
    type Lbl: Label;
    fn get_model(&self) -> &Model<Self::Lbl>;
    fn get_model_mut(&mut self) -> &mut Model<Self::Lbl>;
}
impl<L: Label> ModelWrapper for Model<L> {
    type Lbl = L;

    fn get_model(&self) -> &Model<Self::Lbl> {
        self
    }

    fn get_model_mut(&mut self) -> &mut Model<Self::Lbl> {
        self
    }
}

impl<T> Store for T
where
    T: ModelWrapper + Dom,
{
    fn new_literal(&mut self, presence: Lit) -> Lit {
        self.get_model_mut().state.new_optional_var(0, 1, presence).geq(1)
    }
    fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> VarRef {
        self.get_model_mut().state.new_optional_var(lb, ub, presence)
    }
    fn get_implicant(&mut self, e: ReifExpr) -> Lit {
        self.get_model_mut().half_reify(e.clone())
    }

    fn add_implies(&mut self, l: Lit, e: ReifExpr) {
        self.get_model_mut().enforce_if(l, e);
    }

    fn conjunctive_scope(&mut self, presence_variables: &[Lit]) -> Lit {
        self.get_model_mut().get_conjunctive_scope(presence_variables)
    }
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit {
        self.get_model_mut().get_tautology_of_scope(scope)
    }
}
