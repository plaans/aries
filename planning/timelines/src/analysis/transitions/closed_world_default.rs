use std::collections::{HashMap, HashSet};

use aries_solver::{core::IntCst, lang::Lit};

use super::EffectBasicInfo;
use crate::{EffectId, EffectOp, IntTerm, SchedEncoder, StateVar, encoder::CondId};

/// Closed-world default (ground) initial effects in place of those omitted in (or "missing" from) the main encoding
/// after being derived as unable to support any condition (see [`add_closed_world_negative_effects`]).
///
/// Note that the values of the effects we're "recovering" here do not need to match those of the "original" omitted initial effects,
/// because if they were needed, they wouldn't have been pruned in the main encoding.
///
/// Effect ids below `first_id` correspond to the "original" effects of the encoding.
#[derive(Clone, Default)]
pub(super) struct ClosedWorldDefaultEffects {
    first_id: EffectId,
    store: Vec<EffectBasicInfo>,
    pub ignored_fluents: HashSet<crate::Sym>,
    original_initial_effects_ground_args: HashMap<crate::Sym, Vec<smallvec::SmallVec<[IntCst; 4]>>>,
}
impl ClosedWorldDefaultEffects {
    pub fn new(
        ctx: &SchedEncoder,
        effects_to_ignore: impl IntoIterator<Item = EffectId>,
        conditions_to_ignore: impl IntoIterator<Item = CondId>,
        mut original_initial_effects_ground_args: HashMap<crate::Sym, Vec<smallvec::SmallVec<[IntCst; 4]>>>,
    ) -> Self {
        for (_, entry) in original_initial_effects_ground_args.iter_mut() {
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
            original_initial_effects_ground_args,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
    pub fn contains(&self, eff_id: EffectId) -> bool {
        eff_id >= self.first_id && eff_id < self.first_id + self.store.len()
    }
    pub fn get(&self, offset_eff_id: EffectId) -> &EffectBasicInfo {
        &self.store[offset_eff_id - self.first_id]
    }
    pub fn add(
        &mut self,
        fluent: crate::Sym,
        args: impl Into<smallvec::SmallVec<[IntCst; 4]>>,
        value: IntCst,
    ) -> Result<EffectId, ()> {
        let args = args.into();

        // Ignore if there already is a (non-ignored) initial effect with these ground args.
        if self
            .original_initial_effects_ground_args
            .get(&fluent)
            .is_some_and(|known_grs| known_grs.contains(&args))
        {
            return Err(());
        }

        let eff_view = EffectBasicInfo {
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
