//! Merging of an erase effect and a later assignment of the same action into a single durative effect.
//!
//! An action that *holds* a state variable, e.g.
//!
//! ```text
//!   [start]          available(?x)
//!   [start..]        available(?x) := ∅
//!   [end..]          available(?x) := true
//! ```
//!
//! is rewritten with a single effect `[start, end+dt] available(?x) := true`, whose transition covers the whole
//! interval during which the state variable is held. This is typically much easier to handle for the solver, as
//! the state variable is now exclusively owned by a single effect over the interval.
//!
//! # Conditions
//!
//! A pair `(E₁, E₂)` of effects of an action `A` is *mergeable* if:
//!  - (P1) `E₁` is an erase and `E₂` an assignment, both unconditional and not quantified, on the same state
//!    variable `sv` (syntactically the same simple arguments);
//!  - (P2) `E₂` transitions after `E₁`: `end(E₁) ≤ start(E₂)` in any instance of `A`
//!    (see [`ActionTiming`]);
//!  - (P3) `E₁` is *guarded*: `A` requires `sv` to be defined at `start(E₁)` (see [`required_state_variables`]);
//!  - (P4) the pairing is unambiguous: `E₂` is the only assignment satisfying (P1, P2) for `E₁` and vice versa.
//!
//! The pairs on a fluent `f` are merged only if:
//!  - (F1) every effect on `f` of any action is either guarded, or the assignment of a mergeable pair;
//!  - (F2) no effect on `f` is an increase or decrease;
//!  - (F3) all effects on `f` in the problem definition are at the time origin (no timed initial literals).
//!
//! The pair is rewritten as `E₂` with its transition start set to `start(E₁)`, and `E₁` is removed.
//!
//! # Why this is correct
//!
//! Let `s₁ = start(E₁)`, and `e₂ = end(E₂)`. In the original model, the value of `sv` is undefined over `]s₁, e₂[`
//! (erased then transitioning) and `E₂`'s value afterward, which is exactly the case in the rewritten model.
//! The rewritten model is thus more constrained: its only additional constraint is that
//! no other effect on `sv` may occur within `]s₁, e₂[`. We show that this never happens in a valid plan of the original model.
//!
//! Suppose a valid plan where some effect `E'` on `sv` (other than `E₁`/`E₂`) starts at `t' ∈ ]s₁, e₂[` for a pair
//! `(E₁, E₂)`, and consider such a *violation* with the smallest `t'`. By (F1) and (F3):
//!  - either `E'` is guarded: `sv` must be defined at `t'`. As it is undefined after `s₁`, there is some effect
//!    that gives it a value between `s₁` and `t'`, which is an earlier violation.
//!  - or `E'` is the assignment of another pair `(E₁', E')`, with `s₁' < t'` (P2). If `s₁ < s₁'`, `E₁'` is an
//!    earlier violation of `(E₁, E₂)`. Otherwise, `s₁' < s₁` (they cannot be simultaneous) and `E₁` (guarded by (P3))
//!    is a violation of `(E₁', E')` occurring at `s₁ < t'`.
//!
//! In both cases there is an earlier violation, a contradiction. Effects starting before `s₁` cannot overlap `]s₁, e₂[`
//! as they would also overlap `E₁` in the original model, and increase/decrease effects are excluded by (F2).

use std::collections::{HashMap, HashSet};

use itertools::Itertools;

use crate::preprocessing::analysis::{
    ActionTiming, NumericBounds, RequiredStateVariable, Term, required_state_variables,
};
use crate::*;

/// Merges erase effects with a later assignment of the same state variable into a single durative effect,
/// when no other effect may occur in between (see the module documentation).
/// Returns the number of merged pairs.
pub fn merge_erase_assign_effects(model: &mut Model) -> Res<usize> {
    let env = &model.env;
    let bounds = NumericBounds::new(model);

    // fluents on which no pair can be merged
    let mut invalid: HashSet<FluentId> = HashSet::new();
    // (F3)
    for eff in &model.init {
        if eff.effect_expression.transition_start != Timestamp::ORIGIN {
            invalid.insert(eff.effect_expression.state_variable.fluent);
        }
    }

    // for each action, the mergeable pairs of its effects as `(erase_index, assign_index)`
    let mut pairs: Vec<Vec<(usize, usize)>> = Vec::new();
    for action in model.actions.iter() {
        let info = &ActionInfo {
            timing: ActionTiming::new(action, &bounds),
            requirements: required_state_variables(action, env),
            env,
        };
        let effects = &action.effects;
        let pairable = &|eff: &Effect, erase: bool| {
            eff.effect_expression.condition.is_none()
                && eff.universal_quantification.is_empty()
                && matches!(eff.effect_expression.operation, EffectOp::Erase) == erase
                && matches!(eff.effect_expression.operation, EffectOp::Erase | EffectOp::Assign(_))
        };
        // candidate assignments for each guarded erase (P1, P2, P3)
        let candidates = (effects.iter().enumerate())
            .filter(|(_, e1)| pairable(e1, true) && info.is_guarded(e1))
            .flat_map(|(i, e1)| {
                let sv = info.state_variable(e1);
                (effects.iter().enumerate())
                    .filter(move |(_, e2)| {
                        pairable(e2, false)
                            && info.state_variable(e2) == sv
                            && (info.timing).le(
                                e1.effect_expression.transition_end,
                                e2.effect_expression.transition_start,
                            )
                    })
                    .map(move |(j, _)| (i, j))
            })
            .collect_vec();
        // (P4)
        let action_pairs = (candidates.iter())
            .filter(|(i, j)| candidates.iter().filter(|(i2, j2)| i2 == i || j2 == j).count() == 1)
            .copied()
            .collect_vec();
        // (F1, F2)
        for (j, eff) in effects.iter().enumerate() {
            let fluent = eff.effect_expression.state_variable.fluent;
            let is_paired_assign = action_pairs.iter().any(|(_, j2)| *j2 == j);
            match eff.effect_expression.operation {
                EffectOp::Increase(_) | EffectOp::Decrease(_) => {
                    invalid.insert(fluent);
                }
                EffectOp::Assign(_) | EffectOp::Erase if !is_paired_assign && !info.is_guarded(eff) => {
                    invalid.insert(fluent);
                }
                _ => {}
            }
        }
        pairs.push(action_pairs);
    }

    let mut merged: HashMap<FluentId, usize> = HashMap::new();
    for (action, pairs) in model.actions.iter_mut().zip_eq(pairs) {
        let fluent = |i: usize| action.effects[i].effect_expression.state_variable.fluent;
        let pairs = pairs
            .into_iter()
            .filter(|(i, _)| !invalid.contains(&fluent(*i)))
            .collect_vec();
        for &(i, j) in &pairs {
            *merged
                .entry(action.effects[i].effect_expression.state_variable.fluent)
                .or_default() += 1;
            action.effects[j].effect_expression.transition_start = action.effects[i].effect_expression.transition_start;
        }
        let erases: HashSet<usize> = pairs.iter().map(|(i, _)| *i).collect();
        let mut idx = 0;
        action.effects.retain(|_| {
            idx += 1;
            !erases.contains(&(idx - 1))
        });
    }

    if !merged.is_empty() {
        println!("Merging erase/assign effects into durative effects:");
        for (fluent, count) in merged
            .iter()
            .sorted_by_key(|(f, _)| model.env.fluents.get(**f).name().to_string())
        {
            println!(
                "  - {:<40} [{count} pairs]",
                model.env.fluents.get(*fluent).name().to_string()
            );
        }
    }
    Ok(merged.values().sum())
}

struct ActionInfo<'a> {
    timing: ActionTiming,
    requirements: Vec<RequiredStateVariable>,
    env: &'a Environment,
}

impl ActionInfo<'_> {
    /// The state variable affected by an (unquantified) effect, if its arguments are simple terms.
    fn state_variable(&self, eff: &Effect) -> Option<(FluentId, Vec<Term>)> {
        if !eff.universal_quantification.is_empty() {
            return None;
        }
        let sv = &eff.effect_expression.state_variable;
        Some((sv.fluent, Term::all_of(&sv.arguments, self.env)?))
    }

    /// True if the action requires the state variable of the effect to be defined when its transition starts.
    fn is_guarded(&self, eff: &Effect) -> bool {
        let Some((fluent, args)) = self.state_variable(eff) else {
            return false;
        };
        let t = eff.effect_expression.transition_start;
        (self.requirements.iter())
            .any(|r| r.fluent == fluent && r.args == args && self.timing.holds_before(r.interval, t))
    }
}

#[cfg(test)]
mod tests {
    use crate::pddl::{build_model, input::Input, parser};
    use crate::preprocessing::erase_unusable_effects;
    use crate::*;

    fn merge(domain: &str, problem: &str) -> Model {
        let domain = parser::parse_pddl_domain(Input::from_string(domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        let mut model = build_model(&domain, &problem).unwrap();
        erase_unusable_effects(&mut model).unwrap();
        super::merge_erase_assign_effects(&mut model).unwrap();
        println!("{model}");
        model
    }

    /// Effects of the action on the fluent, as `(transition_start, transition_end, is_erase)`.
    fn effects_on(model: &Model, action: &str, fluent: &str) -> Vec<(TimeRef, TimeRef, bool)> {
        let action = model.actions.get_action(&Sym::from(action)).unwrap();
        (action.effects.iter())
            .map(|eff| &eff.effect_expression)
            .filter(|eff| model.env.fluents.get(eff.state_variable.fluent).name() == fluent)
            .map(|eff| {
                (
                    eff.transition_start.reference,
                    eff.transition_end.reference,
                    matches!(eff.operation, EffectOp::Erase),
                )
            })
            .collect()
    }

    fn domain(extra: &str) -> String {
        format!(
            "(define (domain d) (:requirements :durative-actions :typing)
               (:types r)
               (:predicates (available ?x - r) (done ?x - r))
               (:durative-action use :parameters (?x - r) :duration (= ?duration 5)
                 :condition (at start (available ?x))
                 :effect (and (at start (not (available ?x))) (at end (available ?x)) (at end (done ?x))))
               {extra})"
        )
    }

    const PB: &str = "(define (problem p) (:domain d) (:objects a b - r)
        (:init (available a) (available b)) (:goal (and (done a) (done b))))";

    #[test]
    fn merge_held_resource() {
        let model = merge(&domain(""), PB);
        use TimeRef::*;
        assert_eq!(
            effects_on(&model, "use", "available"),
            [(ActionStart, ActionEnd, false)]
        );
    }

    #[test]
    fn no_merge_with_unguarded_effect() {
        // `reset` may make `available` true while it is held by `use`
        let reset = "(:durative-action reset :parameters (?x - r) :duration (= ?duration 1)
                       :condition (and) :effect (at end (available ?x)))";
        let model = merge(&domain(reset), PB);
        use TimeRef::*;
        assert_eq!(
            effects_on(&model, "use", "available"),
            [(ActionStart, ActionStart, true), (ActionEnd, ActionEnd, false)]
        );
    }

    #[test]
    fn no_merge_with_zero_duration() {
        // the duration is null: the erase and the assignment are simultaneous
        let domain = domain("").replace("(= ?duration 5)", "(= ?duration 0)");
        let model = merge(&domain, PB);
        assert_eq!(effects_on(&model, "use", "available").len(), 2);
    }
}
