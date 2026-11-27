//! Replacement of unusable effects by [`EffectOp::Erase`] effects.
//!
//! An assignment effect is *unusable* if the value it assigns can never be read by any part of the model.
//! Such an effect can be replaced by an `Erase` effect (that leaves the state variable without any value
//! until the next assignment), without changing the set of valid plans nor their quality.
//!
//! Two cases are detected:
//!  - a fluent that is never read (see [`fluents_read`]): all its assignments are unusable.
//!    Fluents that are the target of an increase or decrease effect are left untouched, because erasing
//!    their assignments would leave the increase/decrease effects without a base value.
//!  - a boolean fluent that only appears as a positive atom of requirements
//!    (see [`fluents_with_non_positive_occurrences`]): the assignments to `false` (e.g. PDDL delete effects) are unusable,
//!    because the only thing a condition can require on it is to be `true`, which an erased value cannot satisfy either.
//!
//! Unusable initial effects (at the time origin) are removed entirely rather than erased: the state variable has
//! no value before its first effect anyway. Unusable timed effects of the problem definition (e.g. timed initial
//! literals) are erased, since removing them would leave the previous value in place.

use std::collections::HashSet;

use crate::preprocessing::analysis::{fluents_read, fluents_with_non_positive_occurrences};
use crate::*;

/// Replaces all unusable effects (of actions and of the problem definition) by `Erase` effects, and removes
/// unusable initial effects (see the module documentation). Returns the number of erased or removed effects.
pub fn erase_unusable_effects(model: &mut Model) -> Res<usize> {
    let read = fluents_read(model);
    let non_positive = fluents_with_non_positive_occurrences(model);
    let increased: HashSet<FluentId> = (model.actions.iter().flat_map(|a| &a.effects))
        .chain(&model.init)
        .filter(|eff| {
            matches!(
                eff.effect_expression.operation,
                EffectOp::Increase(_) | EffectOp::Decrease(_)
            )
        })
        .map(|eff| eff.effect_expression.state_variable.fluent)
        .collect();

    let env = &model.env;
    let is_unusable = |eff: &Effect| -> bool {
        let eff = &eff.effect_expression;
        let fluent = eff.state_variable.fluent;
        let EffectOp::Assign(value) = eff.operation else {
            return false;
        };
        if !read.contains(&fluent) {
            !increased.contains(&fluent)
        } else {
            env.fluents.get(fluent).return_type == Type::Bool
                && !non_positive.contains(&fluent)
                && matches!(env.node(value).expr(), Expr::Bool(false))
        }
    };

    let mut num_erased = 0;
    let mut erase = |eff: &mut Effect| {
        if is_unusable(eff) {
            eff.effect_expression.operation = EffectOp::Erase;
            num_erased += 1;
        }
    };
    model
        .actions
        .iter_mut()
        .flat_map(|a| &mut a.effects)
        .for_each(&mut erase);
    let num_init = model.init.len();
    model
        .init
        .retain(|eff| !eff.effect_expression.is_initial() || !is_unusable(eff));
    let num_removed = num_init - model.init.len();
    model.init.iter_mut().for_each(&mut erase);
    Ok(num_erased + num_removed)
}

#[cfg(test)]
mod tests {
    use crate::pddl::{build_model, input::Input, parser};
    use crate::*;

    fn erase(domain: &str, problem: &str) -> Model {
        let domain = parser::parse_pddl_domain(Input::from_string(domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        let mut model = build_model(&domain, &problem).unwrap();
        super::erase_unusable_effects(&mut model).unwrap();
        println!("{model}");
        model
    }

    /// For each effect of the action, the name of the fluent and whether it is an erase.
    fn effects(model: &Model, action: &str) -> Vec<(String, bool)> {
        let action = model.actions.get_action(&Sym::from(action)).unwrap();
        (action.effects.iter())
            .map(|eff| {
                let eff = &eff.effect_expression;
                let name = model.env.fluents.get(eff.state_variable.fluent).name().to_string();
                (name, matches!(eff.operation, EffectOp::Erase))
            })
            .collect()
    }

    const DOMAIN: &str = "(define (domain d) (:requirements :negative-preconditions)
        (:predicates (at ?x) (visited ?x) (clean ?x) (log ?x))
        (:action move :parameters (?from ?to)
          :precondition (and (at ?from) (not (clean ?to)))
          :effect (and (not (at ?from)) (at ?to) (not (visited ?from)) (visited ?to) (log ?to) (not (log ?from)))))";

    const PB: &str = "(define (problem p) (:domain d) (:objects a b)
        (:init (at a) (log a)) (:goal (visited b)))";

    #[test]
    fn erase_unusable() {
        let model = erase(DOMAIN, PB);
        assert_eq!(
            effects(&model, "move"),
            [
                // `at` is only required to be true: the delete effect is unusable
                ("at".to_string(), true),
                ("at".to_string(), false),
                // `visited` is only required to be true (in the goal): the delete effect is unusable
                ("visited".to_string(), true),
                ("visited".to_string(), false),
                // `log` is never read: all its effects are unusable
                ("log".to_string(), true),
                ("log".to_string(), true),
            ]
        );
        // the initial effect on `log` is removed
        let init: Vec<String> = (model.init.iter())
            .map(|eff| {
                model
                    .env
                    .fluents
                    .get(eff.effect_expression.state_variable.fluent)
                    .name()
                    .to_string()
            })
            .collect();
        assert_eq!(init, ["at"]);
    }

    #[test]
    fn keep_negatively_read() {
        let domain = DOMAIN.replace("(not (clean ?to))", "(not (at ?to))");
        let model = erase(&domain, PB);
        // `at` is required to be false: its delete effect must be kept
        assert_eq!(effects(&model, "move")[0], ("at".to_string(), false));
    }
}
