//! Annotation of the *transition* and *borrow* patterns in actions.
//!
//! A **transition** is a condition on a state variable that is immediately followed by an assignment of the same
//! state variable: the condition's interval ends exactly when the effect's transition starts. For instance,
//!
//! ```text
//!   [start]       =(at(?r), ?from)
//!   [start, end]  at(?r) := ?to
//! ```
//!
//! A **borrow** is a special case of a transition where the effect establishes the value required by the condition,
//! e.g. a resource that is required to be available, and is made available again at the end of the effect:
//!
//! ```text
//!   [start]           available(?x)
//!   [start, end+dt]   available(?x) := true
//! ```
//!
//! The conditions and effects are left unchanged: the pairing is recorded in the `transition` field of their
//! annotations, and exposed by [`Action::transition_of_condition`], [`Action::transition_of_effect`] and
//! [`Action::transitions`]. Subsequent transformations must keep these annotations valid
//! (see the [`annotations`](crate::annotations) module).
//!
//! A condition and an effect are paired if they form a transition (see [`Transition`]) and the pairing is
//! unambiguous: the condition forms a transition with no other effect, and the effect with no other condition.

use itertools::Itertools;

use crate::annotations::check_transition;
use crate::*;

/// Annotates the conditions and effects of all actions that form a transition or a borrow (see the module documentation).
///
/// Any previous transition annotation is replaced. Returns the number of transitions found (including borrows).
pub fn annotate_transitions(model: &mut Model) -> Res<usize> {
    let env = &model.env;
    let mut count = 0;
    for action in model.actions.iter_mut() {
        let pairs = transitions(action, env);
        count += pairs.len();
        action
            .conditions
            .iter_mut()
            .for_each(|c| c.annotations.transition = None);
        action.effects.iter_mut().for_each(|e| e.annotations.transition = None);
        for (c, e) in pairs {
            action.conditions[c].annotations.transition = Some(action.effects[e].id);
            action.effects[e].annotations.transition = Some(action.conditions[c].id);
        }
    }
    Ok(count)
}

/// Returns the unambiguous transitions of the action, as `(condition index, effect index)`.
fn transitions(action: &Action, env: &Environment) -> Vec<(usize, usize)> {
    let candidates = (0..action.conditions.len())
        .cartesian_product(0..action.effects.len())
        .filter(|&(i, j)| check_transition(action, &action.conditions[i], &action.effects[j], env).is_some())
        .collect_vec();
    (candidates.iter())
        .filter(|(i, j)| (candidates.iter()).filter(|(i2, j2)| i2 == i || j2 == j).count() == 1)
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::pddl::{build_model, input::Input, parser};
    use crate::preprocessing::{annotate_transitions, erase_unusable_effects, merge_erase_assign_effects};
    use crate::*;

    fn build(domain: &str, problem: &str) -> Model {
        let domain = parser::parse_pddl_domain(Input::from_string(domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        build_model(&domain, &problem).unwrap()
    }

    const DOMAIN: &str = "(define (domain d) (:requirements :durative-actions :typing :negative-preconditions)
        (:types r)
        (:predicates (available ?x - r) (done ?x - r) (fresh ?x - r))
        (:durative-action use :parameters (?x - r) :duration (= ?duration 5)
          :condition (and (at start (available ?x)) (at start (fresh ?x)))
          :effect (and (at start (not (available ?x))) (at end (available ?x))
                       (at start (not (fresh ?x))) (at end (done ?x))))
        (:action refresh :parameters (?x - r)
          :precondition (not (fresh ?x)) :effect (fresh ?x)))";

    const PB: &str = "(define (problem p) (:domain d) (:objects a - r)
        (:init (available a) (fresh a)) (:goal (done a)))";

    /// A model with the transitions of [`DOMAIN`] annotated.
    fn annotated() -> Model {
        let mut model = build(DOMAIN, PB);
        erase_unusable_effects(&mut model).unwrap();
        merge_erase_assign_effects(&mut model).unwrap();
        assert_eq!(annotate_transitions(&mut model).unwrap(), 3);
        println!("{model}");
        model
    }

    fn fluent_name(model: &Model, eff: &Effect) -> String {
        let fluent = eff.effect_expression.state_variable.fluent;
        model.env.fluents.get(fluent).name().to_string()
    }

    /// The valid transitions of an action, as `(fluent of the effect, is_borrow)`, sorted.
    fn transitions(model: &Model, action: &Action) -> Vec<(String, bool)> {
        let mut ts = (action.transitions(&model.env))
            .map(|t| (fluent_name(model, t.effect), t.borrow))
            .collect::<Vec<_>>();
        ts.sort();
        ts
    }

    fn action<'a>(model: &'a Model, name: &str) -> &'a Action {
        model.actions.get_action(&Sym::from(name)).unwrap()
    }

    fn action_mut<'a>(model: &'a mut Model, name: &str) -> &'a mut Action {
        model.actions.get_action_mut(&Sym::from(name)).unwrap()
    }

    #[test]
    fn transitions_and_borrows() {
        let model = annotated();
        assert_eq!(
            transitions(&model, action(&model, "use")),
            [("available".to_string(), true), ("fresh".to_string(), false)]
        );
        // both sides are consistent
        let a = action(&model, "use");
        for eff in &a.effects {
            let expected = fluent_name(&model, eff) != "done";
            assert_eq!(a.transition_of_effect(eff, &model.env).is_some(), expected);
        }
        // instantaneous action: the condition (at start) and effect (at end) are simultaneous
        assert_eq!(
            transitions(&model, action(&model, "refresh")),
            [("fresh".to_string(), false)]
        );
    }

    #[test]
    fn printing() {
        let model = annotated();
        let printed = format!("{}", model.env.node(action(&model, "use")));
        // content of each section (lines following a `name:` header), empty if the section is absent
        let mut sections: std::collections::HashMap<&str, String> = Default::default();
        let mut current = "";
        for line in printed.lines().map(str::trim) {
            if let Some(name) = line.strip_suffix(':') {
                current = name;
            } else {
                sections.entry(current).or_default().push_str(line);
            }
        }
        let section = |name: &str| sections.get(name).cloned().unwrap_or_default();
        assert!(!section("conditions").contains("available"));
        assert!(!section("effects").contains("available"));
        assert!(section("effects").contains("done"));
        assert_eq!(section("borrows"), "[start, end..] available(?x) =~= true");
        assert_eq!(section("transitions"), "[start..] fresh(?x) : true =~> false");
    }

    #[test]
    fn robust_to_reordering() {
        let mut model = annotated();
        let a = action_mut(&mut model, "use");
        a.effects.reverse();
        a.conditions.reverse();
        assert_eq!(
            transitions(&model, action(&model, "use")),
            [("available".to_string(), true), ("fresh".to_string(), false)]
        );
    }

    #[test]
    fn reannotation_after_removal() {
        // removing an element requires the annotations to be recomputed
        let mut model = annotated();
        let fresh = model.env.fluents.get_by_name("fresh").unwrap();
        let a = action_mut(&mut model, "use");
        a.effects.retain(|e| e.effect_expression.state_variable.fluent != fresh);
        assert_eq!(annotate_transitions(&mut model).unwrap(), 2);
        assert_eq!(
            transitions(&model, action(&model, "use")),
            [("available".to_string(), true)]
        );
    }

    #[test]
    #[should_panic(expected = "absent effect")]
    fn removed_counterpart_is_detected() {
        let mut model = annotated();
        let fresh = model.env.fluents.get_by_name("fresh").unwrap();
        let a = action_mut(&mut model, "use");
        a.effects.retain(|e| e.effect_expression.state_variable.fluent != fresh);
        let _ = transitions(&model, action(&model, "use"));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Invalid transition annotation")]
    fn invalidating_modification_is_detected() {
        let mut model = annotated();
        let a = action_mut(&mut model, "use");
        for eff in &mut a.effects {
            eff.effect_expression.transition_start = Timestamp::from(TimeRef::ActionEnd);
        }
        let _ = transitions(&model, action(&model, "use"));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Duplicated effect id")]
    fn duplication_is_detected() {
        let mut model = annotated();
        let a = action_mut(&mut model, "use");
        let held = a.effects.iter().find(|e| e.annotations.transition.is_some()).unwrap().clone();
        a.effects.push(held);
        let _ = transitions(&model, action(&model, "use"));
    }

    #[test]
    fn no_transition_without_merge() {
        // without merging, `available` is erased at start and assigned at end: no assignment follows the condition
        let mut model = build(DOMAIN, PB);
        erase_unusable_effects(&mut model).unwrap();
        annotate_transitions(&mut model).unwrap();
        assert_eq!(
            transitions(&model, action(&model, "use")),
            [("fresh".to_string(), false)]
        );
    }
}
