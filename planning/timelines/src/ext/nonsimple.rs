use std::collections::HashSet;

use itertools::Itertools;

use crate::{
    EffectId, IntTerm,
    encoder::{CondId, SchedEncoder},
    ext::Source,
};

/// An effect is considered "nonsimple" when one of the following holds:
/// - it is not an assignment (or an erase (?WARNING?))
/// - its non-constant terms (variables) do not all appear in their source's arguments
///   (this can happen if it uses a reified variable that is not part of the source's arguments).
///
/// A condition is considered "nonsimple" when one of the following holds:
/// - its non-constant terms (variables) do not all appear in their source's arguments
///   (just as with nonsimple effects, this can happen if it uses a reified variable that is not part of the source's arguments).
/// - it could be supported by a nonsimple effect (there exists a potential causal link between them).
///
/// These conditions and effects may have to be relaxed / ignored in some usages,
/// e.g. grounding or the LP relaxation, due to their handling being unclear there.
pub fn collect_nonsimple_conditions_and_effects_to_relax(ctx: &SchedEncoder) -> (HashSet<CondId>, HashSet<EffectId>) {
    let nonsimple_effects = collect_nonsimple_effects(ctx);
    let nonsimple_conditions = collect_nonsimple_conditions(&nonsimple_effects, ctx);
    (nonsimple_effects, nonsimple_conditions)
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

fn collect_nonsimple_conditions(nonsimple_effects: &HashSet<EffectId>, ctx: &SchedEncoder) -> HashSet<CondId> {
    let mut res = HashSet::new();

    for cl in ctx.causal_links.get_links() {
        if res.contains(&cl.cond_id) {
            continue;
        }
        let cond = &ctx.causal_links.conditions[cl.cond_id];

        if nonsimple_effects.contains(&cl.eff_id)
            || !all_nonconstant_terms_are_included_in_source_terms(
                cond.state_var.args.iter().chain(&[cond.value]).copied(),
                cond.source,
                ctx,
            )
        {
            res.insert(cl.cond_id);
        }
    }
    res
}

fn all_nonconstant_terms_are_included_in_source_terms(
    mut terms: impl Iterator<Item = IntTerm>,
    src: Source,
    ctx: &SchedEncoder,
) -> bool {
    let mut source_terms = if let Some(task_id) = src {
        ctx.sched.tasks[task_id].args.as_slice()
    } else {
        ctx.sched.global_args.as_slice()
    }
    .iter()
    .map(|(t, _)| *t);
    terms.all(|term| term.is_cst() || source_terms.contains(&term))
}
