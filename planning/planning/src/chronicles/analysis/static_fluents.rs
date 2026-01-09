use crate::chronicles::{concrete, EffectOp, Fluent, Problem, StateVar};
use aries::model::extensions::DomainsExt;
fn is_on_fluent(target_fluent: &Fluent, state_var: &StateVar) -> bool {
    target_fluent == state_var.fluent.as_ref()
}

/// Returns true if the fluent is static: all effects are at the temporal origin with no variables in it.
pub fn is_static(target_fluent: &Fluent, pb: &Problem) -> bool {
    let context = &pb.context;
    // convenience functions
    let is_on_target_fluent = |state_var: &StateVar| is_on_fluent(target_fluent, state_var);
    let effect_is_static_assignment = |eff: &concrete::Effect| -> bool {
        // this effect is unifiable with our state variable, we can only make it static if all variables are bound
        if eff
            .state_var
            .args
            .iter()
            .any(|y| !context.model.var_domain(*y).is_singleton())
        {
            return false;
        }
        // effect must be an assignment of a constant
        if let EffectOp::Assign(value) = eff.operation {
            let (lb, ub) = context.model.int_bounds(value);
            if lb != ub {
                return false;
            }
        } else {
            return false;
        }
        eff.effective_start() == context.origin()
    };
    // sf is the state function that we are evaluating for replacement.
    //  - first check that we are in fact allowed to replace it (it only has static effects and all conditions are convertible)
    //  - then transforms it: build a table with all effects and replace the conditions with table constraints
    let mut template_effects = pb.templates.iter().flat_map(|ch| &ch.chronicle.effects);

    let appears_in_template_effects = template_effects.any(|eff| is_on_target_fluent(&eff.state_var));
    if appears_in_template_effects {
        return false; // not a static state function (appears in template)
    }

    let mut effects = pb.chronicles.iter().flat_map(|ch| ch.chronicle.effects.iter());

    let effects_init_and_bound = effects.all(|eff| {
        if is_on_target_fluent(&eff.state_var) {
            // this effect is unifiable with our state variable, we can only make it static if all variables are bound
            effect_is_static_assignment(eff)
        } else {
            true // not interesting, continue
        }
    });
    if !effects_init_and_bound {
        return false; // not a static state function (appears after INIT or not full defined)
    }

    // check that all conditions for this state variable can be converted to a table entry
    let chronicles = pb
        .templates
        .iter()
        .map(|tempplate| &tempplate.chronicle)
        .chain(pb.chronicles.iter().map(|ch| &ch.chronicle));
    let mut conditions = chronicles.flat_map(|ch| ch.conditions.iter());

    conditions.all(|cond| {
        if is_on_target_fluent(&cond.state_var) {
            // the value of this condition must be transformable to an int
            cond.value.int_view().is_some()
        } else {
            true // not interesting, continue
        }
    })
}
