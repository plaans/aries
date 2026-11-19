//! Merging of an assignment and the following condition requiring the assigned value into a single erasure.
//!
//! An action that *holds* a state variable with an intermediate value, e.g.
//!
//! ```text
//!   [start]          available(?x)
//!   [start..]        available(?x) := false
//!   (start, end]     (not available(?x))
//!   [end..]          available(?x) := true
//! ```
//!
//! is rewritten as
//!
//! ```text
//!   [start]          available(?x)
//!   [start, end]     available(?x) := ∅
//!   [end..]          available(?x) := true
//! ```
//!
//! which can be further merged into `[start, end+dt] available(?x) := true`
//! (see [`merge_erase_assign_effects`](super::merge_erase_assign_effects)).
//!
//! # Semantics
//!
//! See [`merge_conditions`](super::merge_conditions): a condition requires each state variable it reads to have a
//! single establisher over its interval, and the exclusivity windows `[start(E) + dt, mutex_end(E)]` of assignments
//! and erasures of the same state variable may not overlap. The window of an erasure ends at the end of its
//! transition, the one of an assignment ends at any time after it (as long as the value is needed).
//!
//! # Conditions
//!
//! An unconditional and unquantified assignment `E₁: [s, s₁] sv := v` (with `s < s₁` and a simple value `v`) of an
//! action `A` is merged with a condition `C: [s₁, e] sv == v` of `A` into an erasure `[s, e] sv := ∅` if:
//!  - (E1) `A` has a single unconditional and unquantified assignment or erasure `E₂: [e, e₂] sv := ...`, with `e < e₂`;
//!  - (E2) if `sv` is boolean, no other effect of `A` on its fluent may start at `s` (it could cancel `E₁` under the
//!    PDDL add-after-delete semantics, which applies to effects on predicates);
//!  - (E3) the fluent is not the target of any increase or decrease effect;
//!  - (E4) all reads of the fluent in the model, but the conditions of `A` on `sv` requiring `v` within `[s₁, e]`
//!    (that are *absorbed*, including `C`), are conditions `X` (or required goals) of the form `sv'`, `(not sv')`
//!    or `(= sv' w)` such that:
//!    - (E4a) the required value is provably distinct from `v`, or
//!    - (E4b) `X` forms a [transition](crate::Transition) with an effect `E'` of its action (other than `E₂` if it is
//!      `A`) with `start(E') < end(E')`.
//!
//! Absorbed conditions are removed.
//!
//! # Why this is correct
//!
//! In the original model, `C` must be established by `E₁`: the window of any establisher of `C` contains `s₁`, as
//! does the window of `E₁`. Hence the window of `E₁` contains `[s + dt, e]`, and is disjoint from the one of
//! `E₂`, starting at `e + dt` (E1): it is exactly `[s + dt, e]`, as for the erasure.
//! The exclusivity constraints are thus the same in both models, and so are the PDDL mutex constraints on effects
//! (an effect of another action whose transition meets `[s₁, e]` would have a window overlapping the one of `E₁` or
//! `E₂`).
//!
//! The only difference is that, in the original model, `E₁` may establish `v` for other conditions within `[s₁, e]`
//! (in the rewritten model, nothing can establish a value there). Absorbed conditions are implied by `C`.
//! A condition `X` requiring another value (E4a) cannot be established by `E₁`. A condition `X` (E4b) ending at `t`
//! and established by `E₁` has `t ≤ e`, while the window of `E'`, starting at `t + dt`, must be disjoint from the
//! one of `E₁`: `t ≥ e`. But with `t = e`, the window of `E'` overlaps the one of `E₂`, a contradiction.
//!
//! Increase and decrease effects are excluded by (E3) as they rely on the window of an assignment.
//! Under the PDDL set semantics, deletes of an action may be cancelled by its simultaneous adds, which (E2) excludes.

use std::collections::{HashMap, HashSet};

use itertools::Itertools;

use crate::annotations::check_transition;
use crate::preprocessing::analysis::{
    ActionTiming, ExprContext, NumericBounds, RequiredValue, SimpleValue, Term, action_roots, conjuncts, model_roots,
    required_value, requires_other_value, requires_value, state_variables,
};
use crate::*;

/// Merges assignments with the following conditions requiring the assigned value into erasures
/// (see the module documentation). Returns the number of merges.
pub fn merge_effect_conditions(model: &mut Model) -> Res<usize> {
    let env = &model.env;
    let bounds = NumericBounds::new(model);
    let timings = model
        .actions
        .iter()
        .map(|a| ActionTiming::new(a, &bounds))
        .collect_vec();
    let reads = Reads::new(model, &timings);

    // for each action, the merges as (assignment index, end of the erasure, absorbed condition indices)
    let mut merges: Vec<Vec<(usize, Timestamp, Vec<usize>)>> = Vec::new();
    for (action_id, action) in model.actions.iter().enumerate() {
        let timing = &timings[action_id];
        let mut action_merges: Vec<(usize, Timestamp, Vec<usize>)> = Vec::new();
        for (i, e1) in action.effects.iter().enumerate() {
            let Some((sv, v)) = simple_assignment(e1, env) else {
                continue;
            };
            let (s, s1) = (
                e1.effect_expression.transition_start,
                e1.effect_expression.transition_end,
            );
            if !timing.lt(s, s1) || reads.stepped.contains(&sv.0) || reads.opaque.contains(&sv.0) {
                continue;
            }
            for c in &action.conditions {
                if !(on_sv(c, &sv, env)).is_some_and(|req| requires_value(req, v, env))
                    || !timing.eq(c.interval.start, s1)
                    || !timing.le(c.interval.start, c.interval.end)
                {
                    continue;
                }
                let e = c.interval.end;
                // (E1)
                let ends = (action.effects.iter().enumerate())
                    .filter(|(_, eff)| {
                        let x = &eff.effect_expression;
                        state_variable(eff, env).as_ref() == Some(&sv)
                            && matches!(x.operation, EffectOp::Assign(_) | EffectOp::Erase)
                            && timing.eq(x.transition_start, e)
                            && timing.lt(x.transition_start, x.transition_end)
                    })
                    .map(|(j, _)| j)
                    .collect_vec();
                let [j] = ends[..] else { continue };
                // (E2)
                let is_bool = env.fluents.get(sv.0).return_type == Type::Bool;
                let cancellable = is_bool
                    && (action.effects.iter().enumerate()).any(|(k, eff)| {
                        let x = &eff.effect_expression;
                        k != i
                            && x.state_variable.fluent == sv.0
                            && may_unify(&x.state_variable.arguments, &sv.1, env)
                            && !timing.lt(x.transition_start, s)
                            && !timing.lt(s, x.transition_start)
                    });
                if cancellable {
                    continue;
                }
                let absorbed = (action.conditions.iter().enumerate())
                    .filter(|(_, x)| {
                        on_sv(x, &sv, env).is_some_and(|req| requires_value(req, v, env))
                            && timing.le(s1, x.interval.start)
                            && timing.le(x.interval.end, e)
                    })
                    .map(|(k, _)| k)
                    .collect_vec();
                // (E4)
                let exploitable = (reads.of(sv.0)).any(|r| {
                    let absorbed = r.action == Some(action_id) && r.condition.is_some_and(|k| absorbed.contains(&k));
                    let other_value = requires_other_value(r.required, v, env);
                    let transition = r.transitions.iter().any(|&t| r.action != Some(action_id) || t != j);
                    !absorbed && !other_value && !transition
                });
                if exploitable {
                    continue;
                }
                let used = |m: &(usize, Timestamp, Vec<usize>)| m.0 == i || m.2.iter().any(|k| absorbed.contains(k));
                if !action_merges.iter().any(used) {
                    action_merges.push((i, e, absorbed));
                    break;
                }
            }
        }
        merges.push(action_merges);
    }

    let mut merged: HashMap<FluentId, usize> = HashMap::new();
    for (action, action_merges) in model.actions.iter_mut().zip_eq(merges) {
        let mut removed = HashSet::new();
        for (i, e, absorbed) in action_merges {
            let eff = &mut action.effects[i].effect_expression;
            eff.operation = EffectOp::Erase;
            eff.transition_end = e;
            removed.extend(absorbed);
            *merged.entry(eff.state_variable.fluent).or_default() += 1;
        }
        let mut k = 0;
        action.conditions.retain(|_| {
            k += 1;
            !removed.contains(&(k - 1))
        });
    }
    if !merged.is_empty() {
        println!("Merging held values into erasures:");
        for (fluent, count) in merged
            .iter()
            .sorted_by_key(|(f, _)| model.env.fluents.get(**f).name().to_string())
        {
            let name = model.env.fluents.get(*fluent).name().to_string();
            println!("  - {name:<40} [{count} actions]");
        }
    }
    Ok(merged.values().sum())
}

/// State variable with simple arguments.
type Sv = (FluentId, Vec<Term>);

/// The state variable and value of an unconditional and unquantified assignment of a simple value.
fn simple_assignment(eff: &Effect, env: &Environment) -> Option<(Sv, ExprId)> {
    let EffectOp::Assign(v) = eff.effect_expression.operation else {
        return None;
    };
    let sv = state_variable(eff, env)?;
    SimpleValue::of(v, env)?;
    Some((sv, v))
}

/// The state variable of an unconditional and unquantified effect, if its arguments are simple.
fn state_variable(eff: &Effect, env: &Environment) -> Option<Sv> {
    let x = &eff.effect_expression;
    if x.condition.is_some() || !eff.universal_quantification.is_empty() {
        return None;
    }
    Some((x.state_variable.fluent, Term::all_of(&x.state_variable.arguments, env)?))
}

/// If the condition is on the state variable, the value it requires.
fn on_sv(cond: &Condition, sv: &Sv, env: &Environment) -> Option<RequiredValue> {
    let (x, required) = required_value(cond.cond, env)?;
    match env.node(x).expr() {
        Expr::StateVariable(f, args) if *f == sv.0 && Term::all_of(args, env).as_ref() == Some(&sv.1) => Some(required),
        _ => None,
    }
}

/// False if the arguments are guaranteed to denote distinct objects.
fn may_unify(args: &[ExprId], terms: &[Term], env: &Environment) -> bool {
    args.iter()
        .zip(terms)
        .all(|(&a, t)| Term::of(a, env).is_none_or(|a| a.may_unify(t)))
}

/// A read of a fluent, as the state variable of a simple requirement.
struct Read {
    /// The action whose condition it is, if any (otherwise a goal).
    action: Option<usize>,
    /// Index of the condition in the action.
    condition: Option<usize>,
    required: RequiredValue,
    /// Indices of the effects forming a transition with the condition (with a non-instantaneous transition).
    transitions: Vec<usize>,
}

/// All reads of fluents in the model.
struct Reads {
    reads: Vec<(FluentId, Read)>,
    /// Fluents that are read in other places than simple requirements.
    opaque: HashSet<FluentId>,
    /// Fluents that are the target of an increase or decrease effect.
    stepped: HashSet<FluentId>,
}

impl Reads {
    fn new(model: &Model, timings: &[ActionTiming]) -> Self {
        let env = &model.env;
        let mut res = Reads {
            reads: Vec::new(),
            opaque: HashSet::new(),
            stepped: HashSet::new(),
        };
        let fluent_of = |sv: ExprId| match env.node(sv).expr() {
            Expr::StateVariable(f, _) => Some(*f),
            _ => None,
        };
        let opaque = |root: ExprId, opaque: &mut HashSet<FluentId>| {
            opaque.extend(state_variables(root, env).into_iter().filter_map(fluent_of));
        };
        // records a simple requirement, or all fluents of the expression as opaque, returns true in the former case
        let add = |expr: ExprId, read: &dyn Fn(RequiredValue) -> Read, res: &mut Reads| {
            let Some((sv, required)) = required_value(expr, env) else {
                opaque(expr, &mut res.opaque);
                return;
            };
            let node = env.node(sv);
            let Expr::StateVariable(fluent, args) = node.expr() else {
                unreachable!()
            };
            args.iter().for_each(|&a| opaque(a, &mut res.opaque));
            if let RequiredValue::Expr(w) = required {
                opaque(w, &mut res.opaque);
            }
            res.reads.push((*fluent, read(required)));
        };

        for (root, ctx) in model_roots(model) {
            match ctx {
                ExprContext::Requirement => {
                    for c in conjuncts(root, env) {
                        let read = |required| Read {
                            action: None,
                            condition: None,
                            required,
                            transitions: Vec::new(),
                        };
                        add(c, &read, &mut res);
                    }
                }
                ExprContext::Other => opaque(root, &mut res.opaque),
            }
        }
        for eff in &model.init {
            if matches!(
                eff.effect_expression.operation,
                EffectOp::Increase(_) | EffectOp::Decrease(_)
            ) {
                res.stepped.insert(eff.effect_expression.state_variable.fluent);
            }
        }
        for (action_id, action) in model.actions.iter().enumerate() {
            let timing = &timings[action_id];
            for (k, cond) in action.conditions.iter().enumerate() {
                let transitions = (action.effects.iter().enumerate())
                    .filter(|(_, eff)| {
                        let x = &eff.effect_expression;
                        timing.lt(x.transition_start, x.transition_end)
                            && check_transition(timing, cond, eff, env).is_some()
                    })
                    .map(|(j, _)| j)
                    .collect_vec();
                let read = |required| Read {
                    action: Some(action_id),
                    condition: Some(k),
                    required,
                    transitions: transitions.clone(),
                };
                add(cond.cond, &read, &mut res);
            }
            // other roots of the action (task network constraints are requirements but not conditions)
            for (root, _) in action_roots(action) {
                if !action.conditions.iter().any(|c| c.cond == root) {
                    opaque(root, &mut res.opaque);
                }
            }
            for eff in &action.effects {
                if matches!(
                    eff.effect_expression.operation,
                    EffectOp::Increase(_) | EffectOp::Decrease(_)
                ) {
                    res.stepped.insert(eff.effect_expression.state_variable.fluent);
                }
            }
        }
        res
    }

    fn of(&self, fluent: FluentId) -> impl Iterator<Item = &Read> {
        (self.reads.iter()).filter(move |(f, _)| *f == fluent).map(|(_, r)| r)
    }
}

#[cfg(test)]
mod tests {
    use crate::pddl::{build_model, input::Input, parser};
    use crate::preprocessing::{erase_unusable_effects, merge_conditions, merge_erase_assign_effects};
    use crate::*;

    /// The extended lock pattern: `free` is held by `use`.
    const USE: &str = "(:durative-action use :parameters (?x - r) :duration (= ?duration 5)
          :condition (and (at start (free ?x)) (over all (not (free ?x))) (at end (not (free ?x))))
          :effect (and (at start (not (free ?x))) (at end (free ?x)) (at end (done ?x))))";

    fn build(actions: &str) -> Model {
        let domain = format!(
            "(define (domain d) (:requirements :durative-actions :typing :negative-preconditions)
               (:types r) (:predicates (free ?x - r) (done ?x - r)) {actions})"
        );
        let problem = "(define (problem p) (:domain d) (:objects a - r) (:init (free a)) (:goal (done a)))";
        let domain = parser::parse_pddl_domain(Input::from_string(&domain)).unwrap();
        let problem = parser::parse_pddl_problem(Input::from_string(problem)).unwrap();
        let mut model = build_model(&domain, &problem).unwrap();
        erase_unusable_effects(&mut model).unwrap();
        merge_conditions(&mut model).unwrap();
        model
    }

    /// Effects of `use` on `free` as `(transition_start, transition_end, is_erase)`, and its number of conditions.
    fn use_action(model: &Model) -> (Vec<(TimeRef, TimeRef, bool)>, usize) {
        let action = model.actions.get_action(&Sym::from("use")).unwrap();
        let effects = (action.effects.iter())
            .map(|eff| &eff.effect_expression)
            .filter(|eff| model.env.fluents.get(eff.state_variable.fluent).name() == "free")
            .map(|eff| {
                let erase = matches!(eff.operation, EffectOp::Erase);
                (eff.transition_start.reference, eff.transition_end.reference, erase)
            })
            .collect();
        (effects, action.conditions.len())
    }

    use TimeRef::*;

    #[test]
    fn extended_lock() {
        let mut model = build(USE);
        assert_eq!(super::merge_effect_conditions(&mut model).unwrap(), 1);
        println!("{model}");
        let erased = vec![(ActionStart, ActionEnd, true), (ActionEnd, ActionEnd, false)];
        assert_eq!(use_action(&model), (erased, 1));
        // the erasure and the assignment are contiguous: they can be merged
        assert_eq!(merge_erase_assign_effects(&mut model).unwrap(), 1);
        assert_eq!(use_action(&model), (vec![(ActionStart, ActionEnd, false)], 1));
    }

    #[test]
    fn exploitable_value() {
        // `spy` could rely on `free` being false while `use` holds it
        let spy = "(:action spy :parameters (?x - r) :precondition (not (free ?x)) :effect (done ?x))";
        let mut model = build(&format!("{USE} {spy}"));
        assert_eq!(super::merge_effect_conditions(&mut model).unwrap(), 0);
    }

    #[test]
    fn transition_reading_value() {
        // `release` reads `free` being false but immediately changes it: it cannot happen while `use` holds it
        let release = "(:action release :parameters (?x - r) :precondition (not (free ?x)) :effect (free ?x))";
        let mut model = build(&format!("{USE} {release}"));
        assert_eq!(super::merge_effect_conditions(&mut model).unwrap(), 1);
        // the contiguous pair is merged even though `release` has an unguarded effect on `free`
        assert_eq!(merge_erase_assign_effects(&mut model).unwrap(), 1);
        assert_eq!(use_action(&model), (vec![(ActionStart, ActionEnd, false)], 1));
    }

    #[test]
    fn no_release() {
        // without an effect at the end of the condition, the value `false` remains after the action
        let mut model = build(&USE.replace("(at end (free ?x))", ""));
        assert_eq!(super::merge_effect_conditions(&mut model).unwrap(), 0);
    }
}
