//! Generators based on the domain transition graphs of the fluents (see [`Dtg`]).

use std::collections::BTreeSet;

use crate::preprocessing::analysis::{Dtg, conjuncts, opaque_fluents, simple_requirement};
use crate::*;

use super::{Cmp, Constraint, CountConstraint, CountVars, PlanScope};

/// Maximum number of values of a fluent for building its DTG.
const MAX_VALUES: usize = 2000;
/// Maximum number of value classes considered for the flow balance of a fluent.
const MAX_CLASSES: usize = 100;

pub(super) fn generate(model: &Model, vars: &CountVars, out: &mut Vec<CountConstraint>) {
    let opaque = opaque_fluents(model);
    for dtg in Dtg::all(model, MAX_VALUES) {
        acyclic(model, vars, &dtg, out);
        absorbing(model, vars, &dtg, out);
        balance(model, vars, &dtg, out);
        if !opaque.contains(&dtg.fluent) {
            chains(model, vars, &dtg, out);
        }
    }
}

/// If all changes of the fluent are transitions and its DTG is acyclic, each state variable performs at most
/// `longest_path` transitions.
fn acyclic(model: &Model, vars: &CountVars, dtg: &Dtg, out: &mut Vec<CountConstraint>) {
    if !dtg.writers.is_empty() || dtg.timed_effects || dtg.transitions.is_empty() {
        return;
    }
    let Some(longest) = dtg.longest_path() else { return };
    let name = (&model.env / dtg.fluent).name();
    let terms = dtg.transitions.iter().map(|t| (t.count as i64, t.action.clone()));
    out.push(CountConstraint {
        constraint: Constraint::Linear(vars.linear(
            terms,
            Cmp::Le,
            (dtg.num_state_variables.saturating_mul(longest)) as i64,
        )),
        scope: PlanScope::AllPlans,
        generator: "acyclic-dtg",
        explanation: format!(
            "the transitions of `{name}` form an acyclic graph: at most {longest} transition(s) for each of the {} state variable(s)",
            dtg.num_state_variables
        ),
    });
}

/// True if all effects of the action that are not on the fluent support nothing.
fn pure_writer(model: &Model, action: &ActionRef, fluent: FluentId) -> bool {
    model.actions.get_action(action).is_some_and(|a| {
        a.effects.iter().all(|e| {
            e.effect_expression.state_variable.fluent == fluent
                || matches!(&e.annotations.supports, Some(Supports::Targets(ts)) if ts.is_empty())
        })
    })
}

/// A value `v` is *absorbing* if no effect can change a state variable that has the value `v`: no transition leaves
/// `v` and all writers only write `v`. Then each state variable enters `v` at most once through a transition, and
/// any write of `v` on a state variable that already has the value `v` is not a contribution.
/// Hence the number of entering transitions and of writes by actions that have no other contribution is bounded by
/// the number of state variables not initially at `v`.
fn absorbing(model: &Model, vars: &CountVars, dtg: &Dtg, out: &mut Vec<CountConstraint>) {
    if dtg.timed_effects {
        return;
    }
    let name = (&model.env / dtg.fluent).name();
    for v in 0..dtg.values.len() {
        let leaves = (dtg.transitions.iter()).any(|t| t.edges.iter().any(|&(u, w)| u == v && w != v));
        let writes_other = dtg.writers.iter().any(|w| w.values.iter().any(|&x| x != v));
        if leaves || writes_other {
            continue;
        }
        // transitions that only enter `v`, and writers of `v`
        let mut terms = Vec::new();
        for t in &dtg.transitions {
            if !t.edges.is_empty() && t.edges.iter().all(|&(u, w)| w == v && u != v) {
                terms.push((t.count as i64, t.action.clone()));
            }
        }
        let has_transitions = !terms.is_empty();
        // only *pure* writers (whose other effects contribute nothing): the write of another action may be redundant
        // while the action is useful for other reasons
        let mut pure_writers = false;
        for w in &dtg.writers {
            if pure_writer(model, &w.action, dtg.fluent) {
                terms.push((1, w.action.clone()));
                pure_writers = true;
            }
        }
        if terms.is_empty() {
            continue;
        }
        let scope = if pure_writers {
            PlanScope::NonRedundant
        } else {
            PlanScope::AllPlans
        };
        let not_initially = dtg.num_state_variables.saturating_sub(dtg.initial[v]);
        let quantified = (model.actions.iter())
            .flat_map(|a| &a.effects)
            .any(|e| e.effect_expression.state_variable.fluent == dtg.fluent && !e.universal_quantification.is_empty());
        if quantified {
            continue; // an instance may change several state variables
        }
        out.push(CountConstraint {
            constraint: Constraint::Linear(vars.linear(terms, Cmp::Le, not_initially as i64)),
            scope,
            generator: "absorbing-value",
            explanation: format!(
                "`{name}` never leaves the value {} once reached: each of the {not_initially} state variable(s) not initially at this value is {} at most once",
                dtg.values[v],
                if has_transitions { "changed to it" } else { "usefully set to it" }
            ),
        });
    }
}

/// For a class `C` of values such that each transition either surely enters `C`, surely leaves it or surely does
/// not change membership to `C`: (entering - leaving) = (number of state variables in `C` at the end) - (initially).
fn balance(model: &Model, vars: &CountVars, dtg: &Dtg, out: &mut Vec<CountConstraint>) {
    if !dtg.writers.is_empty() || dtg.timed_effects || dtg.transitions.iter().any(|t| t.count != 1) {
        return;
    }
    let name = (&model.env / dtg.fluent).name();
    let n = dtg.values.len();
    // candidate classes: sources and targets of each action, and single values
    let mut classes: BTreeSet<BTreeSet<usize>> = BTreeSet::new();
    for t in &dtg.transitions {
        classes.insert(t.edges.iter().map(|e| e.0).collect());
        classes.insert(t.edges.iter().map(|e| e.1).collect());
        for &(u, w) in &t.edges {
            classes.insert([u].into());
            classes.insert([w].into());
        }
    }
    let mut seen = Vec::new();
    for class in classes
        .into_iter()
        .filter(|c| !c.is_empty() && c.len() < n)
        .take(MAX_CLASSES)
    {
        // sign of each action: +1 if it enters the class, -1 if it leaves it, 0 otherwise
        let mut terms = Vec::new();
        let mut decided = true;
        for t in &dtg.transitions {
            let signs: BTreeSet<i64> = (t.edges.iter())
                .map(|(u, w)| class.contains(w) as i64 - class.contains(u) as i64)
                .collect();
            match signs.len() {
                0 => {}
                1 => terms.push((*signs.first().unwrap(), t.action.clone())),
                _ => {
                    decided = false;
                    break;
                }
            }
        }
        if !decided || terms.iter().all(|(s, _)| *s == 0) {
            continue;
        }
        let net = vars.sum(terms.iter().cloned());
        // the complement of a class yields the opposite constraint
        let opposite = vars.sum(terms.iter().map(|(c, a)| (-c, a.clone())));
        if seen.contains(&net) || seen.contains(&opposite) {
            continue;
        }
        seen.push(net.clone());
        let init: u64 = class.iter().map(|&v| dtg.initial[v]).sum();
        let goal_in: u64 = class.iter().map(|&v| dtg.goals[v]).sum();
        let goal_out: u64 = (0..n).filter(|v| !class.contains(v)).map(|v| dtg.goals[v]).sum();
        let lb = goal_in as i64 - init as i64;
        let ub = dtg.num_state_variables.saturating_sub(goal_out) as i64 - init as i64;
        let values = class.iter().map(|&v| dtg.values[v].to_string()).collect::<Vec<_>>();
        let class_name = if values.len() > 6 {
            format!("{{{}, ... ({} values)}}", values[..6].join(", "), values.len())
        } else {
            format!("{{{}}}", values.join(", "))
        };
        let explanation = format!(
            "net number of `{name}` state variables entering {class_name}: {init} initially, between {goal_in} and {} at the end",
            dtg.num_state_variables.saturating_sub(goal_out)
        );
        for (cmp, rhs) in [(Cmp::Ge, lb), (Cmp::Le, ub)] {
            out.push(CountConstraint {
                constraint: Constraint::Linear(net.clone().into_constraint(cmp, rhs).canonical()),
                scope: PlanScope::AllPlans,
                generator: "flow-balance",
                explanation: explanation.clone(),
            });
        }
    }
}

/// In a non-redundant plan without cancelling actions, the transitions of *pure* actions (whose only contribution
/// is their transitions on the fluent) never revisit a value of a state variable between two *uses* of it
/// (a requirement on the fluent, a goal atom, or a writer). Hence at most `D` such transitions per use, where `D` is
/// the maximum length of a simple path in the DTG.
fn chains(model: &Model, vars: &CountVars, dtg: &Dtg, out: &mut Vec<CountConstraint>) {
    let env = &model.env;
    let fluent = dtg.fluent;
    let name = (env / fluent).name();
    let on_fluent = |e: &Effect| e.effect_expression.state_variable.fluent == fluent;
    // pure transition actions: all their effects that are not transitions on the fluent support nothing
    let mut pure = Vec::new();
    for t in &dtg.transitions {
        let Some(a) = model.actions.get_action(&t.action) else {
            continue;
        };
        let Some(status) = a.annotations.status else { return };
        if !status.reachable || !status.useful {
            continue;
        }
        let is_pure = a.effects.iter().all(|e| {
            (on_fluent(e) && a.transition_of_effect(e, env).is_some())
                || matches!(&e.annotations.supports, Some(Supports::Targets(ts)) if ts.is_empty())
        });
        if is_pure {
            pure.push((t.count as i64, a));
        }
    }
    if pure.is_empty() {
        return;
    }
    let d = dtg.max_simple_path() as i64;
    let is_pure = |a: &Action| pure.iter().any(|(_, p)| p.name == a.name);
    let mut terms: Vec<(i64, ActionRef)> = pure.iter().map(|(c, a)| (*c, a.name.clone())).collect();
    let mut users = BTreeSet::new();
    for b in model.actions.iter() {
        for c in &b.conditions {
            // the transition conditions of pure actions are not uses
            if is_pure(b) && b.transition_of_condition(c, env).is_some_and(|t| on_fluent(t.effect)) {
                continue;
            }
            let reads = (conjuncts(c.cond, env).into_iter())
                .filter(|&e| simple_requirement(e, env).is_some_and(|r| r.fluent == fluent))
                .count() as i64;
            if reads > 0 {
                terms.push((-d * reads, b.name.clone()));
                users.insert(b.name.to_string());
            }
        }
    }
    for w in &dtg.writers {
        terms.push((-d, w.action.clone()));
        users.insert(format!("{} (writer)", w.action));
    }
    let goals: u64 = dtg.goals.iter().sum();
    let lin = vars.linear(terms, Cmp::Le, d * goals as i64);
    if !pure.iter().any(|(_, p)| vars.coeff(&lin, &p.name) > 0) {
        return;
    }
    let mut uses = users.into_iter().collect::<Vec<_>>().join(", ");
    if goals > 0 {
        uses = format!("{uses}{}{goals} goal atom(s)", if uses.is_empty() { "" } else { " + " });
    }
    out.push(CountConstraint {
        constraint: Constraint::Linear(lin),
        scope: PlanScope::NonRedundantPlus,
        generator: "transition-chain",
        explanation: format!(
            "at most {d} transition(s) of `{name}` (longest simple path) by pure transition actions before each use: {uses}"
        ),
    });
}
