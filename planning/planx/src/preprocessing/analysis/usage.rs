use std::collections::HashSet;

use crate::*;

use super::{ExprContext, Term, children, conjuncts, expression_roots, state_variables};

/// Returns the fluents that appear in an expression somewhere else than as a *positive* atom of a requirement
/// (condition, goal, constraint).
///
/// An atom is positive if it is only nested in conjunctions, disjunctions, quantifiers and consequents of implications.
/// Fluents that are not in the returned set only appear in places where they are required to be true.
pub fn fluents_with_non_positive_occurrences(model: &Model) -> HashSet<FluentId> {
    let mut found = HashSet::new();
    for (root, ctx) in expression_roots(model) {
        visit(root, ctx == ExprContext::Requirement, &model.env, &mut found);
    }
    found
}

fn visit(expr: ExprId, positive: bool, env: &Environment, found: &mut HashSet<FluentId>) {
    match env.node(expr).expr() {
        Expr::StateVariable(fluent, args) => {
            if !positive {
                found.insert(*fluent);
            }
            for &arg in args {
                visit(arg, false, env, found);
            }
        }
        Expr::App(Fun::And | Fun::Or, args) => {
            for &arg in args {
                visit(arg, positive, env, found);
            }
        }
        Expr::App(Fun::Implies, args) if args.len() == 2 => {
            visit(args[0], false, env, found);
            visit(args[1], positive, env, found);
        }
        Expr::Exists(_, e) | Expr::Forall(_, e) => visit(*e, positive, env, found),
        _ => {
            for child in children(expr, env) {
                visit(child, false, env, found);
            }
        }
    }
}

/// Returns all fluents that are the target of at least one action effect.
///
/// The fluents *not* in this set are static: their value is entirely determined by the problem definition.
pub fn fluents_modified_by_actions(model: &Model) -> HashSet<FluentId> {
    model
        .actions
        .iter()
        .flat_map(|a| &a.effects)
        .map(|eff| eff.effect_expression.state_variable.fluent)
        .collect()
}

/// Returns all fluents that are read somewhere in the model, i.e., that appear in at least one expression
/// (condition, goal, constraint, preference, effect condition/argument/value, duration, metric, ...).
///
/// The fluents *not* in this set are never read: their value has no impact on the validity or quality of a plan.
/// Note that the target state variable of an effect is not a read.
pub fn fluents_read(model: &Model) -> HashSet<FluentId> {
    expression_roots(model)
        .into_iter()
        .flat_map(|(root, _)| state_variables(root, &model.env))
        .filter_map(|e| match model.env.node(e).expr() {
            Expr::StateVariable(fluent, _) => Some(*fluent),
            _ => None,
        })
        .collect()
}

/// A state variable whose value an action requires to be defined over an interval,
/// because a condition of the action reads it as a top-level conjunct (e.g. `p(?x)` or `(= f(?x) ?v)`).
#[derive(Clone, Debug)]
pub struct RequiredStateVariable {
    pub interval: TimeInterval,
    pub fluent: FluentId,
    pub args: Vec<Term>,
}

/// Returns all state variables (with simple arguments) that the action requires to be defined, with the interval
/// over which they are required (see [`RequiredStateVariable`]).
pub fn required_state_variables(action: &Action, env: &Environment) -> Vec<RequiredStateVariable> {
    let mut res = Vec::new();
    let mut push = |interval: TimeInterval, sv: ExprId| {
        if let Expr::StateVariable(fluent, args) = env.node(sv).expr()
            && let Some(args) = Term::all_of(args, env)
        {
            res.push(RequiredStateVariable {
                interval,
                fluent: *fluent,
                args,
            });
        }
    };
    for c in &action.conditions {
        for e in conjuncts(c.cond, env) {
            match env.node(e).expr() {
                Expr::StateVariable(..) => push(c.interval, e),
                Expr::App(Fun::Eq, args) if args.len() == 2 => args.iter().for_each(|&a| push(c.interval, a)),
                _ => {}
            }
        }
    }
    res
}
