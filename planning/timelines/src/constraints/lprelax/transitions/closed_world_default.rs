use std::collections::{HashMap, HashSet};

use aries_solver::prelude::*;

use super::EffectView;
use crate::SchedEncoder;
use crate::{EffectId, EffectOp, IntTerm, StateVar, encoder::CondId};

/// Closed-world default (ground) initial effects in place of those omitted in the main encoding (or "missing" from it)
/// due to being deemed non-required to support any condition [`add_closed_world_negative_effects`]
///
/// The LP relaxation needs them to be a bit stronger (equality rather than upper bound in the inflow constraint on effects,
/// as for the equality to hold we must ensure that flow conservation has a source of flow (one of these "missing" default initial effects)
/// to enter other effects (from actions) on the same state variable).
///
/// Note that the values of the effects we're "recovering" here do not need to match those of the "original" omitted initial effects,
/// because (i) if they were needed, they wouldn't have been pruned in the main encoding,
/// and (ii) we only need them to support other effects, and (condeff/eff)-eff supports to do not care about the value.
///
/// Effect ids below `first_id` correspond to the "original" effects of the encoding.
#[derive(Clone, Default)]
pub(super) struct ClosedWorldDefaultEffects {
    first_id: EffectId,
    store: Vec<EffectView>,
    pub ignored_fluents: HashSet<crate::Sym>,
    initial_effects_ground_args: HashMap<crate::Sym, Vec<Vec<IntCst>>>,
}
impl ClosedWorldDefaultEffects {
    pub fn new(
        ctx: &SchedEncoder,
        effects_to_ignore: impl IntoIterator<Item = EffectId>,
        conditions_to_ignore: impl IntoIterator<Item = CondId>,
        mut initial_effects_ground_args: HashMap<crate::Sym, Vec<Vec<IntCst>>>,
    ) -> Self {
        for (_, entry) in initial_effects_ground_args.iter_mut() {
            debug_assert!({
                use itertools::Itertools;
                entry.iter().all_unique()
            });
            entry.sort_unstable();
        }

        Self {
            first_id: ctx.sched.effects.iter().count(),
            store: vec![],
            ignored_fluents: HashSet::<crate::Sym>::from_iter(
                effects_to_ignore
                    .into_iter()
                    .map(|e_id| ctx.sched.effects.get(e_id).state_var.fluent.clone())
                    .chain(
                        conditions_to_ignore
                            .into_iter()
                            .map(|c_id| ctx.causal_links.conditions.get(c_id).state_var.fluent.clone()),
                    ),
            ),
            initial_effects_ground_args,
        }
    }
    pub fn contains(&self, eff_id: EffectId) -> bool {
        eff_id >= self.first_id && eff_id < self.first_id + self.store.len()
    }
    pub fn get(&self, offset_eff_id: EffectId) -> &EffectView {
        &self.store[offset_eff_id - self.first_id]
    }
    pub fn add(&mut self, fluent: crate::Sym, args: Vec<IntCst>, value: IntCst) -> Result<EffectId, ()> {
        // Ignore if there already is a (non-ignored) initial effect with these ground args.
        if self
            .initial_effects_ground_args
            .get(&fluent)
            .is_some_and(|known_grs| known_grs.contains(&args))
        {
            return Err(());
        }

        let eff_view = EffectView {
            state_var: StateVar {
                fluent,
                args: args.into_iter().map(IntTerm::int_cst).collect(),
            },
            operation: EffectOp::Assign(IntTerm::int_cst(value)),
            prez: Lit::TRUE,
            source: None,
        };

        debug_assert!(
            eff_view
                .state_var
                .args
                .iter()
                .chain(match &eff_view.operation {
                    crate::EffectOp::Assign(term) => [term],
                    crate::EffectOp::Step(_term) => todo!(),
                })
                .all(|term| term.is_cst())
        );

        self.store.push(eff_view);
        Ok(self.first_id + self.store.len() - 1)
    }
}
