//! Simple requirements: the atoms of conditions and goals whose support can be reasoned upon syntactically.

use std::collections::{HashMap, HashSet};

use crate::*;

use super::{ExprContext, SimpleValue, Term, conjuncts, expression_roots, required_value, state_variables};

/// A *simple requirement*: a top-level conjunct `sv`, `(not sv)`, `(= sv v)` or `(= v sv)` of a condition or goal,
/// where all arguments of `sv` and the value `v` are parameters, constants or literals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleRequirement {
    pub fluent: FluentId,
    pub args: Vec<Term>,
    /// The required value (`Bool(true)` for `sv`, `Bool(false)` for `(not sv)`).
    pub value: SimpleValue,
}

/// The simple requirement corresponding to the expression, if it is one.
pub fn simple_requirement(expr: ExprId, env: &Environment) -> Option<SimpleRequirement> {
    let (sv, value) = required_value(expr, env)?;
    let node = env.node(sv);
    let Expr::StateVariable(fluent, args) = node.expr() else {
        return None;
    };
    Some(SimpleRequirement {
        fluent: *fluent,
        args: Term::all_of(args, env)?,
        value: SimpleValue::required(value, env)?,
    })
}

/// The simple requirements of an expression (one per conjunct), or `None` if any of its conjuncts is not a simple
/// requirement.
pub fn simple_requirements(expr: ExprId, env: &Environment) -> Option<Vec<SimpleRequirement>> {
    conjuncts(expr, env)
        .into_iter()
        .map(|c| simple_requirement(c, env))
        .collect()
}

/// The goal expression, if the goal is made of simple requirements only: an unquantified goal that must hold
/// over an interval (e.g. a classical goal at the horizon).
pub fn simple_goal(goal: &Goal, env: &Environment) -> Option<Vec<SimpleRequirement>> {
    match goal.goal_expression {
        SimpleGoal::HoldsDuring(_, e) if goal.universal_quantification.is_empty() => simple_requirements(e, env),
        _ => None,
    }
}

/// Returns the fluents that are read somewhere else than in a simple requirement of an action condition or of
/// a simple goal (see [`simple_goal`]).
///
/// For all other fluents, every read is a simple requirement, on which the support of effects can be determined
/// syntactically.
pub fn opaque_fluents(model: &Model) -> HashSet<FluentId> {
    let env = &model.env;
    let mut opaque = HashSet::new();
    let mark_all = |e: ExprId, opaque: &mut HashSet<FluentId>| {
        for sv in state_variables(e, env) {
            if let Expr::StateVariable(f, _) = env.node(sv).expr() {
                opaque.insert(*f);
            }
        }
    };
    // requirement roots that are handled below (counted, since expressions may be shared)
    let mut handled: HashMap<ExprId, usize> = HashMap::new();
    let mut requirement_roots = Vec::new();
    for action in model.actions.iter() {
        requirement_roots.extend(action.conditions.iter().map(|c| c.cond));
    }
    for goal in &model.goals {
        // other goals are not handled (their expressions are marked as opaque below)
        if let SimpleGoal::HoldsDuring(_, e) = goal.goal_expression
            && goal.universal_quantification.is_empty()
        {
            requirement_roots.push(e);
        }
    }
    for &root in &requirement_roots {
        *handled.entry(root).or_default() += 1;
        for c in conjuncts(root, env) {
            if simple_requirement(c, env).is_none() {
                mark_all(c, &mut opaque);
            }
        }
    }
    for (root, ctx) in expression_roots(model) {
        if ctx == ExprContext::Requirement
            && let Some(n) = handled.get_mut(&root)
            && *n > 0
        {
            *n -= 1;
            continue;
        }
        mark_all(root, &mut opaque);
    }
    opaque
}
