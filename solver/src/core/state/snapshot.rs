use crate::backtrack::{DecLvl, EventIndex};
use crate::core::state::{Domains, Event};
use crate::core::views::Dom;
use crate::core::{IntCst, Lit, SignedVar};
use crate::prelude::*;

/// View of the domains at a given point in time.
///
/// This is primarily intended to query the state as it was when a literal was inferred.
///
/// The class allows constructing either:
///  - a snapshot of the past (typically preceding an inference).
///    Constructing a snapshot of the past is cheap (O(1)) but querying it may be
///    expensive as it need to follow a linked-list of events to find the culprit one.
///  - a snapshot of the current state (mainly there for compatibility,
///    as the snapshot does not bring any added value compared to the wrapped state).
///    Query and construction should remain with a very low overhead
///
/// The type implements the [`Dom`] view trait that exposes many methods through the [`DomainsExt`] extension trait.
pub enum DomainsSnapshot<'a> {
    Current { doms: &'a Domains },
    Past { doms: &'a Domains, next_event: EventIndex },
}

impl<'a> DomainsSnapshot<'a> {
    /// Construct a (low overhead) snapshot of the state as it currently is.
    pub fn current(domains: &'a Domains) -> Self {
        Self::Current { doms: domains }
    }

    /// Builds a simulated reconstruction of the state as it was immediately before the given literal became true.
    ///
    /// Construction is instantaneous but query expensive (O(n) where n is the number of changes to the variable bounds).
    ///
    /// # Panics
    ///
    /// Panics if the literal does not hold or was true in the initial state.
    pub fn preceding(domains: &'a Domains, lit: Lit) -> Self {
        let next_event = domains.implying_event(lit).unwrap();
        Self::Past {
            doms: domains,
            next_event,
        }
    }

    /// Returns the upper bound of the given (signed) variable.
    ///
    /// Note: this is only meant to implement the [`Dom`] trait.
    fn get_upper_bound(&self, var: SignedVar) -> IntCst {
        match self {
            DomainsSnapshot::Current { doms } => doms.ub(var),
            DomainsSnapshot::Past { doms, next_event } => doms
                .doms
                .upper_bounds_history(var)
                .filter(|(_ub, ev)| if let Some(idx) = ev { idx < next_event } else { true })
                .map(|(ub, _)| ub)
                .next()
                .unwrap(),
        }
    }

    /// Returns the presence literal of the variable
    ///
    /// Note: this is only meant to implement the [`Dom`] trait.
    fn get_presence_literal(&self, term: VarRef) -> Lit {
        self.domains().presence(term)
    }

    fn domains(&self) -> &Domains {
        match self {
            DomainsSnapshot::Current { doms } => doms,
            DomainsSnapshot::Past { doms, .. } => doms,
        }
    }

    /// Returns the index of the next event (if a new domain update was done, it would have this index in the trail).
    ///
    /// This is typically useful because a [`DomainsSnapshot`] object is used for explaining and is constructed
    /// to be at the event immediately preceding the inference.
    pub fn next_event(&self) -> EventIndex {
        match self {
            DomainsSnapshot::Current { doms } => doms.trail().next_slot(),
            DomainsSnapshot::Past { next_event, .. } => *next_event,
        }
    }

    pub fn implying_event(&self, l: Lit) -> Option<EventIndex> {
        debug_assert!(self.entails(l));
        self.domains().implying_event(l)
    }

    pub fn get_event(&self, e: EventIndex) -> &Event {
        self.domains().get_event(e)
    }

    pub fn entailing_level(&self, lit: Lit) -> DecLvl {
        self.domains().entailing_level(lit)
    }
}

impl<'a> Dom for DomainsSnapshot<'a> {
    fn upper_bound(&self, svar: SignedVar) -> IntCst {
        self.get_upper_bound(svar)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.get_presence_literal(var)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtrack::Backtrack;
    use crate::core::state::{Cause, InferenceCause};
    use crate::reasoners::ReasonerId;

    /// Dummy cause to mimic propagation
    static INFERENCE: Cause = Cause::Inference(InferenceCause {
        writer: ReasonerId::Diff,
        payload: 0,
    });

    #[test]
    pub fn test_history_access() {
        let max = 77;
        let doms = &mut Domains::new();
        let va = doms.new_var(0, max);
        let vb = doms.new_var(0, max);

        let view = DomainsSnapshot::current(doms);
        assert_eq!(view.ub(va), max);
        assert_eq!(view.ub(vb), max);
        assert_eq!(view.lb(va), 0);
        assert_eq!(view.lb(vb), 0);

        for i in 1..10 {
            doms.save_state();
            doms.set_lb(va, i, Cause::Decision).unwrap();
            doms.set_ub(va, max - 2 * i, INFERENCE).unwrap();
            doms.save_state();
            doms.set_lb(vb, 5 * i, Cause::Decision).unwrap();
            doms.restore_last();
            doms.save_state();
            doms.set_lb(vb, 2 * i, Cause::Decision).unwrap();
            doms.set_ub(vb, max - i, INFERENCE).unwrap();

            let view = DomainsSnapshot::current(doms);
            assert_eq!(view.lb(va), i);
            assert_eq!(view.ub(vb), max - i);

            for j in 1..=i {
                let view = DomainsSnapshot::preceding(doms, Lit::geq(va, j));
                assert_eq!(view.lb(va), j - 1);
                assert_eq!(view.ub(vb), max - j + 1);
                let view = DomainsSnapshot::preceding(doms, Lit::leq(vb, max - j));
                assert_eq!(view.lb(va), j);
                assert_eq!(view.ub(vb), max - j + 1);
            }
        }
    }
}
