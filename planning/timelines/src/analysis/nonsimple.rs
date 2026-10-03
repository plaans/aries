use std::collections::HashSet;

use crate::{
    EffectId, IntTerm,
    analysis::Source,
    encoder::{CondId, SchedEncoder},
};

/// An effect is considered "nonsimple" when one of the following holds:
/// - it is not an assignment (or an erase (?WARNING?))
/// - its non-constant terms (variables) do not all appear in their source's arguments
///   (this can happen if it uses a reified variable that is not part of the source's arguments).
///
/// A condition is considered "nonsimple" when its non-constant terms (variables) do not all appear in their source's arguments
/// (just as with nonsimple effects, this can happen if it uses a reified variable that is not part of the source's arguments).
///
/// Nonsimple-ness then extends to whole fluents: as soon as one effect or condition on a fluent is nonsimple,
/// all the effects and conditions on that fluent are deemed nonsimple too.
/// In particular, both members of a (potential) causal link are either simple or nonsimple together.
///
/// These conditions and effects may have to be relaxed / ignored in some usages,
/// e.g. grounding or the LP relaxation, due to their handling being unclear there.
pub fn collect_nonsimple_conditions_and_effects_to_relax(ctx: &SchedEncoder) -> (HashSet<CondId>, HashSet<EffectId>) {
    // Fluents with at least one nonsimple effect or condition.
    let nonsimple_fluents = std::iter::chain(
        collect_nonsimple_effects(ctx)
            .into_iter()
            .map(|eff_id| ctx.sched.effects.get(eff_id).state_var.fluent.clone()),
        collect_nonsimple_conditions(ctx)
            .into_iter()
            .map(|cond_id| ctx.causal_links.conditions.get(cond_id).state_var.fluent.clone()),
    )
    .collect::<HashSet<_>>();

    let nonsimple_effects = ctx
        .sched
        .effects
        .iter()
        .enumerate()
        .filter(|(_, eff)| nonsimple_fluents.contains(&eff.state_var.fluent))
        .map(|(eff_id, _)| eff_id)
        .collect::<HashSet<_>>();
    let nonsimple_conditions = ctx
        .causal_links
        .conditions
        .iter()
        .enumerate()
        .filter(|(_, cond)| nonsimple_fluents.contains(&cond.state_var.fluent))
        .map(|(cond_id, _)| cond_id)
        .collect::<HashSet<_>>();

    // Both members of a causal link are on the same fluent, so they are either simple or nonsimple together.
    debug_assert!(
        ctx.causal_links
            .get_links()
            .all(|cl| nonsimple_effects.contains(&cl.eff_id) == nonsimple_conditions.contains(&cl.cond_id))
    );

    (nonsimple_conditions, nonsimple_effects)
}

fn collect_nonsimple_effects(ctx: &SchedEncoder) -> HashSet<EffectId> {
    let mut res = HashSet::new();

    for (eff_id, eff) in ctx.sched.effects.iter().enumerate() {
        match eff.operation {
            crate::EffectOp::Assign(term) => {
                if !all_nonconstant_terms_are_included_in_source_terms(
                    eff.state_var.args.iter().chain(&[term]).copied(),
                    eff.source,
                    ctx,
                ) {
                    res.insert(eff_id);
                }
            }
            crate::EffectOp::Step(_term) => {
                res.insert(eff_id);
            }
        }
    }
    res
}

fn collect_nonsimple_conditions(ctx: &SchedEncoder) -> HashSet<CondId> {
    let mut res = HashSet::new();

    for (cond_id, cond) in ctx.causal_links.conditions.iter().enumerate() {
        if !all_nonconstant_terms_are_included_in_source_terms(
            cond.state_var.args.iter().chain(&[cond.value]).copied(),
            cond.source,
            ctx,
        ) {
            res.insert(cond_id);
        }
    }
    res
}

fn all_nonconstant_terms_are_included_in_source_terms(
    mut terms: impl Iterator<Item = IntTerm>,
    src: Source,
    ctx: &SchedEncoder,
) -> bool {
    let source_terms = if let Some(task_id) = src {
        ctx.sched.tasks[task_id].args.as_slice()
    } else {
        ctx.sched.global_args.as_slice()
    };
    terms.all(|term| term.is_cst() || source_terms.contains(&term))
}
