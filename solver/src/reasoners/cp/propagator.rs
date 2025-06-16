// ========== Constraint ===========

use crate::core::state::*;
use crate::core::*;
use crate::create_ref_type;
use crate::reasoners::Contradiction;

use super::Watches;

/// Unique ID of a propagator (assigned by the CP reasoner)
create_ref_type!(PropagatorId);

/// The propagator trait describe the required implementations for implementing a custom propagator in the CP reasoner.
///
/// # Example
///
/// An example propagator is provided in the `propagator::test::implies` in the same file as the trait definition.
pub trait Propagator: Send {
    /// Set up the watches of the propagator, where `id` is the propagator id that should be placed on the watches.
    /// The propagator is responsible for placing a watch on every bound whose change might require a propagation.
    fn setup(&self, id: PropagatorId, context: &mut Watches);

    /// Perform a full-propagation of the constraint.
    ///
    /// Each change in the domains should be annotated with the given `cause` which acts as a signature to determine
    /// that `self` is the propagator that made the inference and should be called for explaining it.
    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction>;

    /// Explain a previous inference made by the constraint.
    ///
    /// The objective is to determine a set of literals `l1, ..., ln` such that `(l1 & l2 & ... & ln) implies `literal`.
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
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &DomainsSnapshot, explanation: &mut Explanation) {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::*;

    mod implies {
        use crate::core::state::*;
        use crate::core::*;
        use crate::reasoners::cp::propagator::*;

        /// An example propagator for an implication constraint (a => b)
        ///
        /// We should propagate (infer) :
        /// - `b` when `a` is true    (case 1)
        /// - `!a` when `b` is false  (case 2)
        #[derive(Clone)]
        pub struct ImpliesProp {
            pub a: Lit,
            pub b: Lit,
        }

        impl Propagator for ImpliesProp {
            fn setup(&self, id: PropagatorId, context: &mut Watches) {
                // request to be notified whenever `a` of `!b` becomes true
                context.add_lit_watch(self.a, id);
                context.add_lit_watch(!self.b, id);
            }

            fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
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

                if self.b.entails(literal) {
                    // b is stronger that `literal`, meaning setting `b` would also have set `literal`
                    debug_assert!(state.entails(self.a), "a was not true at the time we inferred b");
                    out_explanation.push(self.a);
                } else if (!self.a).entails(literal) {
                    debug_assert!(state.entails(!self.b));
                    out_explanation.push(!self.b);
                } else {
                    panic!("Error: we were asked to explain something we could not have inferred")
                }
            }

            fn clone_box(&self) -> Box<dyn Propagator> {
                Box::new(self.clone())
            }
        }
    }
}
