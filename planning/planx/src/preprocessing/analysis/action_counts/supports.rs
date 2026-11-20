//! Generators based on the support annotations (see [`annotate_supports`](crate::preprocessing::annotate_supports)).

use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;

use crate::preprocessing::analysis::{conjuncts, simple_goal};
use crate::*;

use super::{Cmp, Constraint, CountConstraint, CountVars, PlanScope};

pub(super) fn generate(model: &Model, vars: &CountVars, out: &mut Vec<CountConstraint>) {
    dead_actions(model, vars, out);
    goal_demand(model, vars, out);
    required_and_absent(model, vars, out);
    support_capacity(model, vars, out);
}

fn status(model: &Model, a: &ActionRef) -> Option<ActionStatus> {
    model.actions.get_action(a)?.annotations.status
}

/// True if the action may appear in a plan of the scope (according to its status, true if unknown).
fn may_appear(model: &Model, a: &ActionRef, scope: PlanScope) -> bool {
    match status(model, a) {
        None => true,
        Some(s) if scope == PlanScope::AllPlans => s.reachable,
        Some(s) => s.reachable && s.useful,
    }
}

/// Unreachable actions never appear in a plan, useless ones never appear in a non-redundant plan.
fn dead_actions(model: &Model, vars: &CountVars, out: &mut Vec<CountConstraint>) {
    for a in model.actions.iter() {
        let Some(status) = a.annotations.status else { continue };
        let zero = Constraint::Linear(vars.linear([(1, a.name.clone())], Cmp::Eq, 0));
        if !status.reachable {
            out.push(CountConstraint {
                constraint: zero,
                scope: PlanScope::AllPlans,
                generator: "unreachable",
                explanation: "a parameter has no possible value or a requirement can never be supported".to_string(),
            });
        } else if !status.useful {
            out.push(CountConstraint {
                constraint: zero,
                scope: PlanScope::NonRedundant,
                generator: "useless",
                explanation: "no effect can contribute to the goals or metric, even indirectly".to_string(),
            });
        }
    }
}

/// Each goal atom that may not hold initially must be achieved by some action.
/// For each goal fluent, the number of such atoms must be achieved by the instances of their supporting actions.
fn goal_demand(model: &Model, vars: &CountVars, out: &mut Vec<CountConstraint>) {
    let env = &model.env;
    // for each fluent, the supporters of each goal atom not initially true
    let mut groups: BTreeMap<FluentId, Vec<&Supporters>> = BTreeMap::new();
    for g in &model.goals {
        let (Some(reqs), Some(supporters)) = (simple_goal(g, env), g.annotations.supporters.as_ref()) else {
            continue;
        };
        for (req, sup) in reqs.iter().zip(supporters) {
            if !sup.init {
                groups.entry(req.fluent).or_default().push(sup);
            }
        }
    }
    for (fluent, atoms) in groups {
        let name = (env / fluent).name();
        // an effect instance achieves at most one atom, unless it is universally quantified
        let quantified = |e: &EffRef| {
            (model.actions.get_action(&e.action))
                .and_then(|a| a.effect(e.effect))
                .is_none_or(|e| !e.universal_quantification.is_empty())
        };
        if atoms.iter().any(|s| s.effects.iter().any(quantified)) {
            continue;
        }
        let producers = |effects: &mut dyn Iterator<Item = &EffRef>| -> BTreeMap<ActionRef, i64> {
            let distinct: BTreeSet<&EffRef> = effects
                .filter(|e| may_appear(model, &e.action, PlanScope::AllPlans))
                .collect();
            distinct
                .into_iter()
                .map(|e| e.action.clone())
                .counts()
                .into_iter()
                .map(|(a, n)| (a, n as i64))
                .collect()
        };
        // all atoms of the fluent together
        let all = producers(&mut atoms.iter().flat_map(|s| s.effects.iter()));
        let all_producers = all.keys().cloned().collect_vec();
        out.push(CountConstraint {
            constraint: Constraint::Linear(vars.linear(
                all.into_iter().map(|(a, n)| (n, a)),
                Cmp::Ge,
                atoms.len() as i64,
            )),
            scope: PlanScope::AllPlans,
            generator: "goal-demand",
            explanation: format!(
                "{} goal atom(s) on `{name}` not initially true, each instance achieves at most k of them (coefficient)",
                atoms.len()
            ),
        });
        // each atom individually
        for sup in &atoms {
            let ps = producers(&mut sup.effects.iter());
            if ps.keys().eq(all_producers.iter()) {
                continue; // implied by the previous one
            }
            out.push(CountConstraint {
                constraint: Constraint::Linear(vars.linear(ps.into_keys().map(|a| (1, a)), Cmp::Ge, 1)),
                scope: PlanScope::AllPlans,
                generator: "goal-demand",
                explanation: format!("a goal atom on `{name}` is not initially true"),
            });
        }
    }
}

/// If an action is used and one of its conditions cannot hold initially, then some supporter must be used.
fn required_and_absent(model: &Model, vars: &CountVars, out: &mut Vec<CountConstraint>) {
    let env = &model.env;
    for b in model.actions.iter() {
        if !may_appear(model, &b.name, PlanScope::AllPlans) {
            continue;
        }
        for c in &b.conditions {
            let Some(sup) = &c.annotations.supporters else { continue };
            if sup.init {
                continue;
            }
            let producers: BTreeSet<&ActionRef> = (sup.effects.iter())
                .map(|e| &e.action)
                .filter(|a| may_appear(model, a, PlanScope::AllPlans))
                .collect();
            if producers.contains(&b.name) {
                continue; // trivially satisfied
            }
            out.push(CountConstraint {
                constraint: Constraint::Implies {
                    if_used: vars.n(&b.name),
                    then: vars.linear(producers.into_iter().map(|a| (1, a.clone())), Cmp::Ge, 1),
                },
                scope: PlanScope::AllPlans,
                generator: "required-absent",
                explanation: format!("condition `{}` cannot hold initially", env / c),
            });
        }
    }
}

/// In a non-redundant plan, each instance of an action is the last establisher of at least one requirement instance
/// (of another action or a goal atom), and each requirement instance has a single last establisher.
/// Thus `n(a) <= sum over the requirements r that a may support of the number of instances of r`.
fn support_capacity(model: &Model, vars: &CountVars, out: &mut Vec<CountConstraint>) {
    let env = &model.env;
    for a in model.actions.iter() {
        if !may_appear(model, &a.name, PlanScope::NonRedundant) {
            continue;
        }
        // targets of all effects: condition -> undo (only if all effects targeting it are undos)
        let mut conditions: BTreeMap<CondRef, bool> = BTreeMap::new();
        let mut goals: BTreeMap<GoalId, usize> = BTreeMap::new();
        let mut bounded = true;
        for e in &a.effects {
            match &e.annotations.supports {
                None | Some(Supports::Opaque) => bounded = false,
                Some(Supports::Targets(targets)) => {
                    for t in targets {
                        match t {
                            SupportTarget::Condition { cond, undo } => {
                                let entry = conditions.entry(cond.clone()).or_insert(true);
                                *entry &= *undo;
                            }
                            SupportTarget::Goal { goal, atoms } => *goals.entry(*goal).or_default() += atoms,
                        }
                    }
                }
            }
        }
        if !bounded {
            continue;
        }
        // goal atoms, each counted at most once
        let goal_atoms: usize = (goals.iter())
            .map(|(g, n)| {
                let size = (model.goals.iter())
                    .find(|goal| goal.id == *g)
                    .and_then(|goal| goal.annotations.supporters.as_ref())
                    .map_or(*n, |s| s.len());
                (*n).min(size)
            })
            .sum();
        for scope in [PlanScope::NonRedundant, PlanScope::NonRedundantPlus] {
            let exclude_undo = scope == PlanScope::NonRedundantPlus;
            if exclude_undo && !conditions.values().any(|undo| *undo) {
                continue;
            }
            let mut terms = vec![(1, a.name.clone())];
            let mut readers = BTreeSet::new();
            for (cond, undo) in &conditions {
                if (exclude_undo && *undo) || !may_appear(model, &cond.action, PlanScope::NonRedundant) {
                    continue;
                }
                // each instance of the condition has as many requirements as conjuncts
                let weight = (model.actions.get_action(&cond.action))
                    .and_then(|b| b.condition(cond.condition))
                    .map_or(1, |c| conjuncts(c.cond, env).len().max(1)) as i64;
                terms.push((-weight, cond.action.clone()));
                readers.insert(cond.action.to_string());
            }
            let lin = vars.linear(terms, Cmp::Le, goal_atoms as i64);
            // skip constraints where the action supports itself (no bound)
            if vars.coeff(&lin, &a.name) <= 0 {
                continue;
            }
            let mut readers = readers.into_iter().join(", ");
            if goal_atoms > 0 {
                readers = format!(
                    "{readers}{}{goal_atoms} goal atom(s)",
                    if readers.is_empty() { "" } else { " + " }
                );
            }
            out.push(CountConstraint {
                constraint: Constraint::Linear(lin),
                scope,
                generator: "support-capacity",
                explanation: format!(
                    "each instance must establish a value read by {}{}",
                    if readers.is_empty() {
                        "nothing".to_string()
                    } else {
                        readers
                    },
                    if exclude_undo { " (excluding undos)" } else { "" }
                ),
            });
        }
    }
}
