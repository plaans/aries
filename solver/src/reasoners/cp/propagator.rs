pub mod justified;

use std::fmt::Debug;

use crate::core::state::*;
use crate::create_ref_type;
use crate::prelude::*;
use crate::reasoners::Contradiction;

use super::Watches;

// Unique ID of a propagator (assigned by the CP reasoner)
create_ref_type!(PropagatorId);

/// The propagator trait describe the required implementations for implementing a custom propagator in the CP reasoner.
///
/// # Example
///
/// An example propagator is provided in the `propagator::test::implies` in the same file as the trait definition.
/// ```
/// use aries_solver::prelude::*;
/// use aries_solver::core::state::*;
/// use aries_solver::reasoners::*;
/// use aries_solver::reasoners::cp::*;
/// use aries_solver::reasoners::cp::propagator::*;
///
/// /// An example propagator for an implication constraint (a => b)
/// ///
/// /// We should propagate (infer) :
/// /// - `b` when `a` is true    (case 1)
/// /// - `!a` when `b` is false  (case 2)
/// #[derive(Clone, Debug)]
/// pub struct ImpliesProp {
///     pub a: Lit,
///     pub b: Lit,
/// }
///
/// impl Propagator for ImpliesProp {
///     fn setup(&self, id: PropagatorId, context: &mut Watches) {
///         // request to be notified whenever `a` or `!b` becomes true
///         context.add_lit_watch(self.a, id);
///         context.add_lit_watch(!self.b, id);
///     }
///
///     fn propagate(&mut self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
///         if domains.entails(self.a) {
///             // a is true, we should propagate b
///             // we set `b` to true in the domain which wuold return one of:
///             //  - Ok(true): the change was performed sucessfully
///             //  - Ok(false): nothing was done (i.e. b was already true)
///             //  - Err(xx): contradiction (i.e. b was already false!)
///             //
///             // In the first two cases, we proceed.
///             // If an error was returned, the `?` operator will short-circuit and
///             // immediately return with an appropriate `Contradiction`
///             domains.set(self.b, cause)?;
///         }
///         // we did not reach an error, propagate the other case
///         if domains.entails(!self.b) {
///             domains.set(!self.a, cause)?;
///         }
///         // if we reach this point, propagation was successful, return Ok
///         Ok(())
///     }
///
///     fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation) {
///         // we are asked to explain a propagation that we previously made
///
///         if self.b.entails(literal) && state.entails(self.a) {
///             // b is stronger that `literal`, meaning setting `b` would also have set `literal`
///             out_explanation.push(self.a);
///         } else if (!self.a).entails(literal) && state.entails(!self.b) {
///             out_explanation.push(!self.b);
///         } else {
///             panic!("Error: we were asked to explain something we could not have inferred")
///         }
///     }
///
///     fn clone_box(&self) -> Box<dyn Propagator> {
///         Box::new(self.clone())
///     }
/// }
/// ```
pub trait Propagator: Send {
    /// Set up the watches of the propagator, where `id` is the propagator id that should be placed on the watches.
    /// The propagator is responsible for placing a watch on every bound whose change might require a propagation.
    fn setup(&self, id: PropagatorId, context: &mut Watches);

    /// Perform a full-propagation of the constraint.
    ///
    /// Each change in the domains should be annotated with the given `cause` which acts as a signature to determine
    /// that `self` is the propagator that made the inference and should be called for explaining it.
    fn propagate(&mut self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction>;

    /// Explain a previous inference made by the constraint.
    ///
    /// The objective is to determine a set of literals `l1, ..., ln` such that `(l1 & l2 & ... & ln)` implies `literal`.
    /// This literals should be appended to the provided `out_explanation`.
    ///
    /// The `state` parameter provides a view of the `domains` as they were at the time the inference was made.
    ///
    /// Important: `literal` may not be exactly the literal inferred but a weaker one. For instance, if the propagation
    /// inferred `(x <= 6)`, the propagator may be asked to explain the literal `(x <= 7)`.
    ///
    /// Note: Though not needed for correctness, it is in general important to have *minimal* explanation (the smallest possible set of implying literals).
    fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation);

    /// Create a boxed version of the propagator.
    fn clone_box(&self) -> Box<dyn Propagator>;
}

impl<T: Propagator> Explainer for T {
    fn explain(
        &mut self,
        _cause: InferenceCause,
        literal: Lit,
        model: &DomainsSnapshot,
        explanation: &mut Explanation,
    ) {
        Propagator::explain(self, literal, model, explanation)
    }
}

/// A simple wrapper around a propagator for dynamic-dipsatch
pub struct DynPropagator {
    pub(super) constraint: Box<dyn Propagator>,
}

impl Clone for DynPropagator {
    fn clone(&self) -> Self {
        DynPropagator {
            constraint: self.constraint.clone_box(),
        }
    }
}

impl<T: Propagator + 'static> From<T> for DynPropagator {
    fn from(propagator: T) -> Self {
        DynPropagator {
            constraint: Box::new(propagator),
        }
    }
}

/// Trait describing the minimal functionnality a custom propagator needs in order to be added to a [`Model`].
pub trait UserPropagator: Debug + Sync + Send {
    /// Instantiate a new propagator to be integrated into the CP engine.
    fn get_propagators(&self) -> Vec<DynPropagator>;

    /// Returns true iff, the propagator is entailed by the given solution.
    fn satisfied(&self, sol: &Solution) -> bool;
}

#[cfg(test)]
pub mod test {

    mod implies {
        //! Example propagator for an implication

        use crate::core::state::*;
        use crate::reasoners::cp::propagator::*;

        /// An example propagator for an implication constraint (a => b)
        ///
        /// We should propagate (infer) :
        /// - `b` when `a` is true    (case 1)
        /// - `!a` when `b` is false  (case 2)
        #[derive(Clone, Debug)]
        pub struct ImpliesProp {
            pub a: Lit,
            pub b: Lit,
        }

        impl Propagator for ImpliesProp {
            fn setup(&self, id: PropagatorId, context: &mut Watches) {
                // request to be notified whenever `a` or `!b` becomes true
                context.add_lit_watch(self.a, id);
                context.add_lit_watch(!self.b, id);
            }

            fn propagate(&mut self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
                if domains.entails(self.a) {
                    // a is true, we should propagate b
                    // we set `b` to true in the domain which wuold return one of:
                    //  - Ok(true): the change was performed sucessfully
                    //  - Ok(false): nothing was done (i.e. b was already true)
                    //  - Err(xx): contradiction (i.e. b was already false!)
                    //
                    // In the first two cases, we proceed.
                    // If an error was returned, the `?` operator will short-circuit and
                    // immediately return with an appropriate `Contradiction`
                    domains.set(self.b, cause)?;
                }
                // we did not reach an error, propagate the other case
                if domains.entails(!self.b) {
                    domains.set(!self.a, cause)?;
                }
                // if we reach this point, propagation was successful, return Ok
                Ok(())
            }

            fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation) {
                // we are asked to explain a propagation that we previously made

                if self.b.entails(literal) && state.entails(self.a) {
                    // b is stronger that `literal`, meaning setting `b` would also have set `literal`
                    out_explanation.push(self.a);
                } else if (!self.a).entails(literal) && state.entails(!self.b) {
                    out_explanation.push(!self.b);
                } else {
                    panic!("Error: we were asked to explain something we could not have inferred")
                }
            }

            fn clone_box(&self) -> Box<dyn Propagator> {
                Box::new(self.clone())
            }
        }

        //  ===== Tests ======

        use itertools::Itertools;
        use rand::prelude::IndexedRandom;
        use rand::rngs::SmallRng;
        use rand::{Rng, SeedableRng};

        /// Generates `n` random problems, each with a domain with a few variables and a propagator
        fn implies_problems(n: usize) -> Vec<(Domains, ImpliesProp)> {
            let mut rng = SmallRng::seed_from_u64(0);
            let mut problems = Vec::new();

            for _ in 0..n {
                let mut d = Domains::new();
                let num_vars = rng.random_range(2..=10);
                let vars = (0..num_vars).map(|_| d.new_var(0, 10)).collect_vec();
                let a = vars.choose(&mut rng).unwrap().leq(rng.random_range(0..=10));
                let b = vars.choose(&mut rng).unwrap().leq(rng.random_range(0..=10));
                let a = if rng.random_bool(0.5) { a } else { !a };
                let b = if rng.random_bool(0.5) { b } else { !b };
                problems.push((d, ImpliesProp { a, b }));
            }

            problems
        }

        #[test]
        fn test_explanations() {
            use crate::reasoners::cp::testing::*;
            for (d, mut s) in implies_problems(1000) {
                println!("\nConstraint: {s:?}");
                test_explanations(&d, &mut s, true);
            }
        }
    }
}
