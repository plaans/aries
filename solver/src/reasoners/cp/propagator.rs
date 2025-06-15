// ========== Constraint ===========

use crate::core::state::*;
use crate::core::*;
use crate::create_ref_type;
use crate::reasoners::Contradiction;

use super::Watches;

/// Unique ID of a propagator (assigned by the CP reasoner)
create_ref_type!(PropagatorId);

pub trait Propagator: Send {
    fn setup(&self, id: PropagatorId, context: &mut Watches);
    fn propagate(&self, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction>;
    fn propagate_event(&self, _event: &Event, domains: &mut Domains, cause: Cause) -> Result<(), Contradiction> {
        self.propagate(domains, cause)
    }

    fn explain(&self, literal: Lit, state: &DomainsSnapshot, out_explanation: &mut Explanation);

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

    /// An example propagator for an implication constraint (a => b)
    ///
    /// We should propagate (infer) :
    /// - `b` when `a` is true    (case 1)
    /// - `!a` when `b` is false  (case 2)
    #[derive(Clone)]
    struct ImpliesProp {
        a: Lit,
        b: Lit,
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
            }
        }

        fn clone_box(&self) -> Box<dyn Propagator> {
            Box::new(self.clone())
        }
    }
}
