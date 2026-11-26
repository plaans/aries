//! Merging of the conditions of an action that require the same expression over overlapping or contiguous intervals.
//!
//! For instance, the conditions
//!
//! ```text
//!   (start, end)  (not available(?x))
//!   [end]         (not available(?x))
//! ```
//!
//! are replaced by the single condition `(start, end] (not available(?x))`.
//!
//! # Semantics
//!
//! Time is discrete, `dt` being the smallest delay between two timepoints. A condition over `[a, b]` requires each
//! state variable `sv` it reads to have a *single* establisher over the whole interval: an assignment effect `E` with
//! `end(E) ≤ a` whose exclusivity window `[start(E) + dt, mutex_end(E)]` contains `[a, b]`. The exclusivity windows of
//! two assignments (or erasures) of the same state variable do not overlap.
//! Moreover (PDDL mutex), `[a, b]` may not intersect the transition `[start(E'), end(E') - dt]` of an effect `E'` on
//! `sv` of *another* action.
//!
//! # Conditions
//!
//! Two conditions over `[a, b]` and `[c, d]` (with `a ≤ c`, `b ≤ d`) on syntactically identical expressions are merged
//! into a condition over `[a, d]` if:
//!  - (M1) no state variable they read is the target of an increase or decrease effect;
//!  - (M2) the intervals overlap (`c ≤ b`), or they are contiguous (`c = b + dt`) and:
//!    - (M2a) no effect of the action on a fluent they read may start at `b`;
//!    - (M2b) no effect of the model on a fluent they read may have an instantaneous transition (`start = end`),
//!      except at or before the origin (as the initial state and the default values of predicates);
//!    - (M2c) `b` is not before the start of the action, and thus `b + dt` is after the origin.
//!
//! # Why this is correct
//!
//! The merged condition is at least as strong as the original ones, and the PDDL mutex constraints are the same
//! (on a discrete time line, `[a, d]` is the union of `[a, b]` and `[c, d]`). Consider a state variable `sv` read
//! by the conditions, with establishers `E` over `[a, b]` and `E'` over `[c, d]`. If they are the same effect, it
//! establishes the same value over `[a, d]`. Otherwise, their exclusivity windows are disjoint: as `E`'s contains `b`,
//! the one of `E'` starts after `b`: `start(E') ≥ b`, and since `end(E') ≤ c ≤ b + dt`:
//!  - with overlapping intervals (`c ≤ b`), `end(E') ≤ b < start(E') + dt`: impossible.
//!  - if `start(E') = b` and `end(E') = b + dt`, the transition of `E'` meets `[a, b]`: `E'` must be an effect of
//!    the action itself (PDDL mutex), which is excluded by (M2a).
//!  - otherwise `E'` has an instantaneous transition at `b + dt`, excluded by (M2b) and (M2c).
//!
//! Increase/decrease effects (excluded by (M1)) contribute to the conditions whose interval is included in their
//! window, and could thus contribute to only one of the two conditions.

use std::collections::HashSet;

use itertools::Itertools;

use crate::preprocessing::analysis::{ActionTiming, NumericBounds, state_variables, syntactically_equal};
use crate::*;

/// Merges conditions on the same expression with overlapping or contiguous intervals (see the module documentation).
/// Returns the number of merges performed.
pub fn merge_conditions(model: &mut Model) -> Res<usize> {
    let env = &model.env;
    let bounds = NumericBounds::new(model);

    // (M1) fluents that are the target of an increase or decrease effect
    let mut stepped = HashSet::new();
    // (M2b) fluents that may be the target of an effect with an instantaneous transition
    let mut instantaneous = HashSet::new();
    let mut record = |eff: &Effect, timing: &ActionTiming| {
        let e = &eff.effect_expression;
        let fluent = e.state_variable.fluent;
        if matches!(e.operation, EffectOp::Increase(_) | EffectOp::Decrease(_)) {
            stepped.insert(fluent);
        }
        if !e.is_initial() && !timing.lt(e.transition_start, e.transition_end) {
            instantaneous.insert(fluent);
        }
    };
    let global = ActionTiming::global(&bounds);
    model.init.iter().for_each(|eff| record(eff, &global));
    for action in model.actions.iter() {
        let timing = ActionTiming::new(action, &bounds);
        action.effects.iter().for_each(|eff| record(eff, &timing));
    }

    // compute the merges of each action
    let mut merges = Vec::new();
    for action in model.actions.iter() {
        let timing = ActionTiming::new(action, &bounds);
        let mut conditions = action.conditions.clone();
        let mut num_merges = 0;
        while let Some((i, j, interval)) = find_merge(action, &conditions, &timing, &stepped, &instantaneous, env) {
            conditions[i] = Condition::over(interval, conditions[i].cond);
            conditions.remove(j);
            num_merges += 1;
        }
        merges.push((num_merges, conditions));
    }

    let mut count = 0;
    for (action, (num_merges, conditions)) in model.actions.iter_mut().zip_eq(merges) {
        if num_merges > 0 {
            action.conditions = conditions;
            count += num_merges;
        }
    }
    Ok(count)
}

/// Finds two conditions (by index) that can be merged into a single condition over the returned interval.
fn find_merge(
    action: &Action,
    conditions: &[Condition],
    timing: &ActionTiming,
    stepped: &HashSet<FluentId>,
    instantaneous: &HashSet<FluentId>,
    env: &Environment,
) -> Option<(usize, usize, TimeInterval)> {
    for (i, j) in (0..conditions.len()).tuple_combinations() {
        let (c1, c2) = (&conditions[i], &conditions[j]);
        if !syntactically_equal(c1.cond, c2.cond, env) {
            continue;
        }
        let fluents = state_variables(c1.cond, env)
            .into_iter()
            .filter_map(|sv| match env.node(sv).expr() {
                Expr::StateVariable(fluent, _) => Some(*fluent),
                _ => None,
            })
            .collect_vec();
        // (M1)
        if fluents.iter().any(|f| stepped.contains(f)) {
            continue;
        }
        // (M2)
        let Some((interval, switch)) = union(c1.interval, c2.interval, timing) else {
            continue;
        };
        if let Some(b) = switch {
            let may_start_at_b = |eff: &Effect| {
                let ts = eff.effect_expression.transition_start;
                !timing.lt(ts, b) && !timing.lt(b, ts)
            };
            let reads = |eff: &Effect| fluents.contains(&eff.effect_expression.state_variable.fluent);
            if fluents.iter().any(|f| instantaneous.contains(f))
                || !timing.le(Timestamp::from(TimeRef::ActionStart), b)
                || action.effects.iter().any(|eff| reads(eff) && may_start_at_b(eff))
            {
                continue;
            }
        }
        return Some((i, j, interval));
    }
    None
}

/// If the two intervals overlap or are contiguous, returns their union, and, for contiguous intervals,
/// the end `b` of the first one (the next one starting at `b + dt`).
fn union(i1: TimeInterval, i2: TimeInterval, timing: &ActionTiming) -> Option<(TimeInterval, Option<Timestamp>)> {
    let (first, second) = if timing.le(i1.start, i2.start) {
        (i1, i2)
    } else if timing.le(i2.start, i1.start) {
        (i2, i1)
    } else {
        return None;
    };
    let end = if timing.le(second.end, first.end) {
        first.end
    } else if timing.le(first.end, second.end) {
        second.end
    } else {
        return None;
    };
    let interval = TimeInterval {
        start: first.start,
        end,
    };
    if timing.le(second.start, first.end) {
        Some((interval, None))
    } else if timing.eq(second.start, first.end + Dt) {
        Some((interval, Some(first.end)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::pddl::{build_model, input::Input, parser};
    use crate::*;
    use itertools::Itertools;

    fn merge(domain: &str) -> Model {
        let problem = "(define (problem p) (:domain d) (:objects a - r) (:init (p a)) (:goal (done a)))";
        let domain = parser::parse_pddl_domain(Input::from_string(domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        let mut model = build_model(&domain, &problem).unwrap();
        super::merge_conditions(&mut model).unwrap();
        println!("{model}");
        model
    }

    fn domain(conditions: &str, effects: &str) -> String {
        format!(
            "(define (domain d) (:requirements :durative-actions :typing :negative-preconditions)
               (:types r)
               (:predicates (p ?x - r) (done ?x - r))
               (:durative-action act :parameters (?x - r) :duration (= ?duration 5)
                 :condition (and {conditions}) :effect (and (at end (done ?x)) {effects})))"
        )
    }

    /// Intervals of the conditions of `act`, sorted.
    fn intervals(model: &Model) -> Vec<String> {
        let action = model.actions.get_action(&Sym::from("act")).unwrap();
        action
            .conditions
            .iter()
            .map(|c| c.interval.to_string())
            .sorted()
            .collect()
    }

    #[test]
    fn contiguous() {
        let model = merge(&domain("(over all (p ?x)) (at end (p ?x))", ""));
        assert_eq!(intervals(&model), ["(start, end]"]);
    }

    #[test]
    fn contiguous_with_switch() {
        // `p` may be deleted at start and re-established within the action: the two conditions are not equivalent
        // to a condition over `[start, end)`
        let model = merge(&domain(
            "(at start (p ?x)) (over all (p ?x))",
            "(at start (not (p ?x)))",
        ));
        assert_eq!(intervals(&model).len(), 2);
        // without the effect at start, they are merged
        let model = merge(&domain("(at start (p ?x)) (over all (p ?x))", ""));
        assert_eq!(intervals(&model), ["[start, end)"]);
    }

    #[test]
    fn distinct_expressions() {
        let model = merge(&domain("(over all (p ?x)) (at end (not (p ?x)))", ""));
        assert_eq!(intervals(&model).len(), 2);
    }
}
