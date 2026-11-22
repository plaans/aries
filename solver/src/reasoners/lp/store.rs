use aries_lp::prelude::Bound;

use crate::backtrack::DecLvl;
use crate::collections::ref_store::RefVec;
use crate::core::literals::{Conjunction, Watches};
use crate::core::views::Dom;
use crate::core::{Lit, LongCst};

use super::{BoundConstraintId, BoundRestriction};

#[derive(Clone, Default)]
pub(super) struct BoundConstraintsStore {
    /// The bound constraints, by identifier: each one has a trigger (a conjunction of model literals) and a bound restriction,
    /// which is applied to the LP as soon as all the literals of the trigger are entailed.
    pub(super) store: RefVec<BoundConstraintId, (Conjunction, BoundRestriction)>,
    /// The constraints whose cause was already entailed when they were registered,
    /// and that are thus waiting to be applied at the next propagation.
    pending: Vec<BoundConstraintId>,
    /// The pending constraints applied above the root, with the LP level they were applied at (increasing).
    /// No watch can trigger them again, so backtracking below that level makes them pending again.
    applied_pending: Vec<(DecLvl, BoundConstraintId)>,

    /// For each LP variable, the bound constraints on its upper bound (index 0) and on its lower bound (index 1),
    /// each with its bound, sorted by tightest bound first (equal bounds stay in registration order).
    by_var: Vec<[Vec<(LongCst, BoundConstraintId)>; 2]>,
    /// The watches on the literals of the triggers.
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

        // if the trigger is already entailed, enqueue the constraint for the next propagation:
        // the watches only trigger on future events.
        let entailed = trigger.iter().all(|l| domains.entails(l));
        if entailed {
            self.pending.push(id);
        }
        if !entailed || !at_root {
            // unless we are at ROOT AND the trigger is entailed, we place watches to check the trigger whenever one of its literals becomes true
            //
            // Note: if this becomes performance critical, we could use a single watch, always placed on an undecided literal
            // but this would complicate the implementation and is unlikely to be useful in practice, as the vast majority of cases should involve only one/two
            // literals in the conjunction.
            for l in &trigger {
                self.watches.add_watch(id, l);
            }
        }
        let actual_id = self.store.push((trigger, bound_update));
        let var = bound_update.var.idx();
        if self.by_var.len() <= var {
            self.by_var.resize(var + 1, Default::default());
        }
        // inserted to preserve sorting order (tightest bound first)
        let constrs = match bound_update.bound {
            Bound::Upper => &mut self.by_var[var][0],
            Bound::Lower => &mut self.by_var[var][1],
        };
        let pos = constrs.partition_point(|&(val, _)| match bound_update.bound {
            Bound::Upper => val <= bound_update.val,
            Bound::Lower => val >= bound_update.val,
        });
        constrs.insert(pos, (bound_update.val, actual_id));

        debug_assert!(id == actual_id);
        id
    }

    /// The bound constraints on the bound `bound` of the variable `var`, with their bound value,
    /// sorted by tightest one first.
    pub(super) fn constrs_on(&self, var: usize, bound: Bound) -> &[(LongCst, BoundConstraintId)] {
        let Some(constrs) = self.by_var.get(var) else {
            return &[];
        };
        match bound {
            Bound::Upper => &constrs[0],
            Bound::Lower => &constrs[1],
        }
    }

    pub(super) fn watches_on(&self, lit: Lit) -> impl Iterator<Item = BoundConstraintId> {
        self.watches.watches_on(lit)
    }

    /// The constraints whose trigger is entailed in `domains`, among those watching `lit`.
    pub(super) fn triggered_by(&self, lit: Lit, domains: &impl Dom) -> impl Iterator<Item = BoundConstraintId> {
        self.watches_on(lit)
            .filter(|&id| self.store[id].0.iter().all(|l| domains.entails(l)))
    }

    pub(super) fn pop_pending(&mut self) -> Option<BoundConstraintId> {
        self.pending.pop()
    }

    /// Records that a pending constraint was applied at LP level `lvl`.
    pub(super) fn record_applied_pending(&mut self, id: BoundConstraintId, lvl: DecLvl) {
        if lvl > DecLvl::ROOT {
            self.applied_pending.push((lvl, id));
        }
    }

    /// After a backtrack of the LP to `lvl`: the pending constraints applied above it have their bound undone
    /// (or overwritten by an undone older bound), while their causes may still be entailed, so they are pending again.
    pub(super) fn backtrack_to(&mut self, lvl: DecLvl) {
        while let Some(&(applied_at, id)) = self.applied_pending.last() {
            if applied_at <= lvl {
                break;
            }
            self.applied_pending.pop();
            self.pending.push(id);
        }
    }
}
