use crate::backtrack::DecLvl;
use crate::collections::ref_store::RefVec;
use crate::core::Lit;
use crate::core::literals::{Conjunction, Watches};
use crate::core::views::Dom;

use super::{BoundConstraintId, BoundRestriction};

#[derive(Clone, Default)]
pub(super) struct BoundConstraintsStore {
    /// Associates each bound constraint identifiers with a trigger (conjunction of model literals) and a corresponding bound restriction.
    /// the constraint is applied to the LP as soon as all the literals of its trigger are entailed.
    pub(super) store: RefVec<BoundConstraintId, (Conjunction, BoundRestriction)>,
    /// The constraints whose cause was already entailed when they were registered,
    /// and that are thus waiting to be applied at the next propagation.
    pending: Vec<BoundConstraintId>,
    /// Decision level at which the pending constraints were last applied.
    /// No watch can trigger them again, so backtracking below it makes them all pending anew.
    pending_applied_at: Option<DecLvl>,

    by_var: Vec<Vec<BoundConstraintId>>,
    watches: Watches<BoundConstraintId>,
}
impl BoundConstraintsStore {
    /// Registers the bound constraint `trigger -> bound_update`,
    /// given the domains at the LP level, and whether the model is at the root.
    pub(super) fn add(
        &mut self,
        trigger: Conjunction,
        bound_update: BoundRestriction,
        domains: &impl Dom,
        at_root: bool,
    ) -> BoundConstraintId {
        let id = self.store.next_key();

        // check if the trigger is already entailed and in which case, enqueue for immediate propagation.
        // This is necessary because watches would only trigger for future events.
        let entailed = trigger.iter().all(|l| domains.entails(l));
        if entailed {
            self.pending.push(id);
        }
        if !entailed || !at_root {
            // unless we are at ROOT AND the trigger is entailed, we place watch to we can check the trigger if one of its literals becomes true
            //
            // Note: if this becomes performance critical, we could use a single watch, always placed on an undecided literal
            // but this would complicate the implementation and unlikely to be useful in practice as the the vast majority of case should involve only one/two
            // literals in the conjunction.
            for l in &trigger {
                self.watches.add_watch(id, l);
            }
        }
        let actual_id = self.store.push((trigger, bound_update));
        let var = bound_update.var.idx();
        if self.by_var.len() <= var {
            self.by_var.resize(var + 1, vec![]);
        }
        self.by_var[var].push(actual_id);

        debug_assert!(id == actual_id);
        id
    }

    pub(super) fn watches_on(&self, lit: Lit) -> impl Iterator<Item = BoundConstraintId> {
        self.watches.watches_on(lit)
    }

    /// The constraints whose trigger is entailed in `domains`, among those watching `lit`.
    pub(super) fn triggered_by(&self, lit: Lit, domains: &impl Dom) -> impl Iterator<Item = BoundConstraintId> {
        self.watches_on(lit)
            .filter(|&id| self.store[id].0.iter().all(|l| domains.entails(l)))
    }

    /// Mark the pending constraints as applied at decision level `lvl` (without popping them).
    pub(super) fn mark_pending_applied(&mut self, lvl: DecLvl) {
        if !self.pending.is_empty() {
            debug_assert!(self.pending_applied_at.is_none());
            self.pending_applied_at = Some(lvl);
        }
    }

    pub(super) fn pop_pending(&mut self) -> Option<BoundConstraintId> {
        self.pending.pop()
    }

    /// After a backtrack of the LP to `lvl`: if it went below the level the pending constraints were applied at,
    /// their bounds are undone while their causes may still be entailed, so they are all marked pending again.
    pub(super) fn backtrack_to(&mut self, lvl: DecLvl) {
        if self.pending_applied_at.is_some_and(|applied_at| applied_at > lvl) {
            self.pending.clear();
            self.pending.extend(self.store.keys());
            self.pending_applied_at = None;
        }
    }
}
