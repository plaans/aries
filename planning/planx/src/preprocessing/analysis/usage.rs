use std::collections::HashSet;

use crate::*;

use super::{ExprContext, children, expression_roots};

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
