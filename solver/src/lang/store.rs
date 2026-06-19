use std::fmt::Debug;

use crate::model::Model;
use crate::{
    backtrack::DecLvl,
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
    fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> Var;
    fn get_implicant(&mut self, e: ReifExpr) -> Lit;
    fn add_implies(&mut self, l: Lit, e: ReifExpr);

    /// Given a set of scope literals, return a (possibly new) scope literal that
    /// exactly represents the intersection of thoses scope.
    ///
    /// One can retrieve the intersected scope with [`Store::decompose_scope`].
    fn conjunctive_scope(&mut self, lits: &[Lit]) -> Lit;

    /// Returns a literal that is always true and with scope `scope`.
    ///
    /// The purpose of this method is to avoid creating several tautological literals per scope.
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit;

    /// Given a scope literal, returns an equivalent conjunction of scope literals.
    ///
    /// This method is the inverse of [`Store::conjunctive_scope`] and allows breaking an intersection scope into its intersected components.
    /// If the scope is not an interesection scope, it will return the intersection with a single element.
    fn decompose_scope(&self, scope: Lit) -> Conjunction;

    /// Returns the list of literals that are known to be always implied `l`.
    ///
    /// The mehtod may not find all possible implication.
    ///
    /// Currently, it will only detect that for implication explicitly declared with [`Domains::add_implication`] (which is only intended for scope literals).
    fn statically_implied_by(&self, l: Lit) -> impl Iterator<Item = Lit>;

    /// Returns `true` if the literal is tautological in this model (entailed at the root level).
    fn statically_entailed(&self, l: Lit) -> bool;

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

                fn satisfied(&self, sol: &Solution) -> bool {
                    // the extract solution step is unnecessarily costly (copying the domains of each assertion to validate)
                    self.cond.as_ref().evaluate(sol) != Some(false)
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
        self.get_model_mut().new_optional_variable(0, 1, presence).geq(1)
    }
    fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> Var {
        self.get_model_mut().new_optional_variable(lb, ub, presence)
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

    fn decompose_scope(&self, scope: Lit) -> Conjunction {
        self.get_model()
            .shape
            .conjunctive_scopes
            .conjuncts(scope)
            .map(Conjunction::from_iter)
            .unwrap_or(Conjunction::from(scope))
    }

    fn statically_implied_by(&self, l: Lit) -> impl Iterator<Item = Lit> {
        self.get_model().state.implied_by(l)
    }

    fn statically_entailed(&self, l: Lit) -> bool {
        let m = &self.get_model().state;
        m.entails(l) && m.entailing_level(l) == DecLvl::ROOT
    }
}
