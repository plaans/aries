use std::fmt::Debug;

use crate::{
    core::{state::Evaluable, views::Dom},
    model::Label,
    prelude::*,
    reasoners::cp::UserPropagator,
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

    fn enforce_user_propagator(&mut self, propagator: impl UserPropagator + 'static);

    /// Adds a debug assertion on solutions, i.e., an expression that is assumed to always evaluate to true.
    ///
    /// IMPORTANT: the assertion has NO effect on the solving process and only checked when debug assertions are enabled (not in release mode)
    ///
    /// TODO: this could be provided on the `Model` it self to be more generally useful
    #[track_caller]
    fn add_assertion<Expr: Evaluable<Value = bool> + Debug + Send + Sync + 'static>(&mut self, condition: Expr) {
        // The assertion is costly to create and evaluate so it is only active when debug assertions are activated
        if cfg!(debug_assertions) {
            // a specifying `UserPropagator` that never propagates but provides a method to check that it is satified
            struct SolutionAssertion {
                cond: std::sync::Arc<dyn Evaluable<Value = bool> + Send + Sync>,
                debug: String,
                source: String,
            }
            impl Debug for SolutionAssertion {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct("SolutionAssertion")
                        .field("expression", &self.debug)
                        .field("source", &self.source)
                        .finish()
                }
            }

            impl UserPropagator for SolutionAssertion {
                fn get_propagators(&self) -> Vec<crate::reasoners::cp::DynPropagator> {
                    // no propagators to record, so nothing will happen for this constraint
                    vec![]
                }

                fn satisfied(&self, dom: &Domains) -> bool {
                    // the extract solution step is unnecessarily costly (copying the domains of each assertion to validate)
                    self.cond.as_ref().evaluate(&dom.extract_solution()) != Some(false)
                }
            }
            let debug = format!("{condition:?}");
            let caller = std::panic::Location::caller();
            let source = format!("{}:{}", caller.file(), caller.line());
            let checker = SolutionAssertion {
                cond: std::sync::Arc::new(condition),
                debug,
                source,
            };
            self.enforce_user_propagator(checker);
        }
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

    fn enforce_user_propagator(&mut self, propagator: impl UserPropagator + 'static) {
        self.get_model_mut().enforce_user_propagator(propagator);
    }
}
