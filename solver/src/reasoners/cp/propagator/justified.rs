//! Provides convenience types and traits for propagators that must store a justification to be able to explain their inferences.
//!
//! The module introduces the [`JustifiedPropagator`] trait that forces the propagator to provide a justification for each inference.
//! The justification is then passed to the propagator when requiring an explanation for a previous inference.
//!
//! A type implementing [`JustifiedPropagator`] can be turn into a normal [`Propagator`] by wrapping it inside a [`PropagatorWithJustifications`]
//! that will handle the storage of justifications.
//!
//! This kind of propagators are useful when propagation is complex and one want to avoid to reproduce the propagation algorithm for explaining each inference.
//! This is notably used in the `NoOverlap` propagator.
//!
//! Below is a simple example to illustrate how the different parts fit together.
//!
//! ```
//! use aries_solver::prelude::*;
//! use aries_solver::core::state::*;
//! use aries_solver::reasoners::cp::propagator::*;
//! use aries_solver::reasoners::cp::propagator::justified::*;
//! use aries_solver::reasoners::cp::*;
//!
//! /// A dummy propagator on a variable
//! #[derive(Debug, Clone)]
//! struct Prop(Var);
//!
//! /// A justification for `Prop`. Can be any arbitrary type, here just storing an integer
//! #[derive(Debug, Clone)]
//! struct Just(IntCst);
//!
//! impl JustifiedPropagator<Just> for Prop {
//!     fn setup(&self, _id: PropagatorId, _context: &mut Watches) {
//!         todo!("Settup the watches (same as normal propagator)")
//!     }
//!     fn propagate(&mut self, domains: &mut DomainsAndJustifications<Just>) -> Result<(), InvalidUpdate> {
//!         let upper_bound: IntCst = todo!("complex computation");
//!         domains.set(self.0.leq(upper_bound), Just(upper_bound))?;
//!         todo!("When modifying domains, a justification must be provided.")
//!     }
//!     fn explain(
//!         &self,
//!         lit: Lit,
//!         justification: &Just,
//!         domains: &DomainsSnapshot,
//!         out_explanation: &mut Explanation,
//!     ) {
//!         let recorded_value: IntCst = justification.0;
//!         todo!("When explaining a literal, the associated justification is available")
//!     }
//! }
//! // implement the `UserPropagator` trait that will allow posting it to a model
//! impl UserPropagator for Prop {
//!     fn get_propagators(&self) -> Vec<DynPropagator> {
//!         // `Prop` does not readily matches the `Propagator` interface: it needs to be wrapped in inside
//!         // a `PropagatorWithJustifications` struct, that will be responsible for storing and providing the justifications.
//!         let wrapped_propagator = PropagatorWithJustifications::build(self.clone());
//!         vec![wrapped_propagator]
//!     }
//!     fn satisfied(&self, sol: &Solution) -> bool {
//!         todo!("Check if the constraint is satisfied for the current domains")
//!     }
//! }
//! let mut model = Model::new();
//! model.enforce_user_propagator(Prop(Var::ZERO));
//! ```

use crate::{
    backtrack::EventIndex,
    core::{
        state::{DomainsSnapshot, Explanation, InvalidUpdate},
        views::{Boundable, Dom},
    },
    prelude::*,
    reasoners::cp::{DynPropagator, Propagator},
};

/// A trait for implementing a propagator that justifies all its inferences (and requires the justifications to be available for explaining).
///
/// It mirror the [`Propagator`] trait but requires a justification on propagation and provides it when explaning.
pub trait JustifiedPropagator<Justification> {
    fn setup(&self, id: super::PropagatorId, context: &mut crate::reasoners::cp::Watches);
    fn propagate(&mut self, domains: &mut DomainsAndJustifications<Justification>) -> Result<(), InvalidUpdate>;
    fn explain(
        &self,
        lit: Lit,
        justification: &Justification,
        domains: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    );
}

type Hist<J> = Vec<(EventIndex, J)>;

/// A wrapper for a propagator that requires an explicit storage of a justification for each of its inferences.
#[derive(Clone)]
pub struct PropagatorWithJustifications<Prop, Justification> {
    prop: Prop,
    justifications: Hist<Justification>,
}

impl<Prop, Justification> PropagatorWithJustifications<Prop, Justification> {
    pub fn new(propagator: Prop) -> Self {
        PropagatorWithJustifications {
            prop: propagator,
            justifications: Default::default(),
        }
    }

    /// Returns a dynamically dispatched propagator for `propagator`
    pub fn build(propagator: Prop) -> DynPropagator
    where
        Prop: JustifiedPropagator<Justification> + Send + Clone + 'static,
        Justification: Send + Clone + 'static,
    {
        DynPropagator::from(Self::new(propagator))
    }
}

impl<Prop, Justification> Propagator for PropagatorWithJustifications<Prop, Justification>
where
    Prop: JustifiedPropagator<Justification> + Send + Clone + 'static,
    Justification: Send + Clone + 'static,
{
    fn setup(&self, id: super::PropagatorId, context: &mut crate::reasoners::cp::Watches) {
        self.prop.setup(id, context);
    }

    fn propagate(
        &mut self,
        domains: &mut Domains,
        cause: crate::core::state::Cause,
    ) -> Result<(), crate::reasoners::Contradiction> {
        let ev = domains.trail().next_slot();
        while let Some((last, _)) = self.justifications.last() {
            if last < &ev {
                break;
            }
            self.justifications.pop();
        }
        let mut doms = DomainsAndJustifications {
            cause,
            domains,
            justifications: &mut self.justifications,
        };
        let () = self.prop.propagate(&mut doms)?;
        Ok(())
    }

    fn explain(
        &self,
        literal: crate::prelude::Lit,
        state: &crate::core::state::DomainsSnapshot,
        out_explanation: &mut crate::core::state::Explanation,
    ) {
        let ev = state.next_event();
        debug_assert!(self.justifications.is_sorted_by_key(|(ev, _)| ev));
        let Ok(idx) = self.justifications.binary_search_by_key(&ev, |(hist_ev, _)| *hist_ev) else {
            panic!("No justification recorded for this event")
        };
        let justification = &self.justifications[idx].1;
        self.prop.explain(literal, justification, state, out_explanation);
    }

    fn clone_box(&self) -> Box<dyn Propagator> {
        Box::new(self.clone())
    }
}

/// A wrapper type that provides access to to a domain and stores a set of justification for its changes.
///
/// When a modification is made to the domains, a justification should be provided for it which will be stored along if the modification is not a no-op.
pub struct DomainsAndJustifications<'a, Justification> {
    cause: crate::core::state::Cause,
    domains: &'a mut Domains,
    justifications: &'a mut Hist<Justification>,
}

impl<'a, J> Dom for DomainsAndJustifications<'a, J> {
    fn _upper_bound(&self, svar: SignedVar) -> IntCst {
        self.domains._upper_bound(svar)
    }

    fn _presence(&self, var: Var) -> Lit {
        self.domains._presence(var)
    }
}

impl<'a, J> MutDomExt<J> for DomainsAndJustifications<'a, J> {
    fn set(&mut self, literal: Lit, cause: J) -> Result<bool, InvalidUpdate> {
        let ev = self.domains.trail().next_slot();
        // events should always have strictly increasing indices, which is enforced by cleaning up the history
        // when starting a new propagation
        // A case where this may be error prone is for failures: they do not modify the trail so it in theory
        // possible to have multiple at the same level. However in this case, the propagator should exit immediatly
        // (here the debug assert below acts a sanity check)
        debug_assert!(self.justifications.last().iter().all(|(last_ev, _)| last_ev < &ev));
        match self.domains.set(literal, self.cause) {
            Ok(false) => Ok(false), // no changes, there is no need to save the justification
            res => {
                // successful update or failure, record justification to enable explanation
                self.justifications.push((ev, cause));
                res
            }
        }
    }
}

/// Extension trait for structures that allow modifying a domain with a justification.
///
/// TODO: this should be generalized beyond this use-case (all modifications in inferences require a justification of some kind)
pub trait MutDomExt<Justification>: Dom {
    fn set(&mut self, literal: Lit, cause: Justification) -> Result<bool, InvalidUpdate>;

    /// Modifies the lower bound of a variable.
    /// The module that made this modification should be identified in the `cause` parameter, which can
    /// be used to query it for an explanation of the change.
    ///
    /// The function returns:
    ///  - `Ok(true)` if the bound was changed and it results in a valid (non-empty) domain.
    ///  - `Ok(false)` if no modification of the domain was carried out. This might occur if the
    ///    provided bound is less constraining than the existing one.
    ///  - `Err(EmptyDomain(v))` if the change resulted in the variable `v` having an empty domain.
    ///    In general, it cannot be assumed that `v` is the same as the variable passed as parameter.
    #[inline]
    fn set_lb<Var: Boundable>(
        &mut self,
        var: Var,
        new_lb: Var::Value,
        cause: Justification,
    ) -> Result<bool, InvalidUpdate> {
        self.set(var.geq(new_lb), cause)
    }

    /// Modifies the upper bound of a variable.
    /// The module that made this modification should be identified in the `cause` parameter, which can
    /// be used to query it for an explanation of the change.
    ///
    /// The function returns:
    ///  - `Ok(true)` if the bound was changed and it results in a valid (non-empty) domain
    ///  - `Ok(false)` if no modification of the domain was carried out. This might occur if the
    ///    provided bound is less constraining than the existing one.
    ///  - `Err(EmptyDomain(v))` if the change resulted in the variable `v` having an empty domain.
    ///    In general, it cannot be assumed that `v` is the same as the variable passed as parameter.
    #[inline]
    fn set_ub<Var: Boundable>(
        &mut self,
        var: Var,
        new_ub: Var::Value,
        cause: Justification,
    ) -> Result<bool, InvalidUpdate> {
        self.set(var.leq(new_ub), cause)
    }
}
