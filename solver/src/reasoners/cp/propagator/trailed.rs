use crate::{
    backtrack::EventIndex,
    core::{
        state::{DomainsSnapshot, Explanation, InvalidUpdate},
        views::{Boundable, Dom},
    },
    prelude::*,
    reasoners::cp::Propagator,
};

type Hist<J> = Vec<(EventIndex, J)>;

pub struct JustifiedPropagator<Prop, Justification> {
    prop: Prop,
    justifications: Hist<Justification>,
}

impl<Prop, Justification> JustifiedPropagator<Prop, Justification> {
    pub fn new(propagator: Prop) -> Self
    where
        Prop: JustifiedProp<Justification>,
    {
        JustifiedPropagator {
            prop: propagator,
            justifications: Default::default(),
        }
    }
}

impl<Prop, Justification> Propagator for JustifiedPropagator<Prop, Justification>
where
    Prop: JustifiedProp<Justification> + Send,
    Justification: Send,
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
        let mut doms = DomJust {
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
        todo!()
    }
}

pub trait JustifiedProp<Justification> {
    fn setup(&self, id: super::PropagatorId, context: &mut crate::reasoners::cp::Watches);
    fn propagate(&mut self, domains: &mut DomJust<Justification>) -> Result<(), InvalidUpdate>;
    fn explain(
        &self,
        lit: Lit,
        justification: &Justification,
        domains: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    );
}

pub struct DomJust<'a, Justification> {
    cause: crate::core::state::Cause,
    domains: &'a mut Domains,
    justifications: &'a mut Hist<Justification>,
}

impl<'a, J> Dom for DomJust<'a, J> {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.domains.upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.domains.presence(var)
    }
}

impl<'a, J> MutDomExt<J> for DomJust<'a, J> {
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
