use crate::*;

/// Context in which a (root) expression appears in a model.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExprContext {
    /// A statement that must hold for the plan to be valid (condition, goal, constraint).
    Requirement,
    /// Any other usage, where the expression may be evaluated with any polarity or as a non-boolean value
    /// (preference, effect condition or value, duration, metric, task argument, ...).
    Other,
}

/// Returns all root expressions of the model (their sub-expressions are not listed), together with their context.
///
/// Any expression of the model is either in this list or a sub-expression of an element of this list.
pub fn expression_roots(model: &Model) -> Vec<(ExprId, ExprContext)> {
    let mut roots = model_roots(model);
    for action in model.actions.iter() {
        roots.extend(action_roots(action));
    }
    roots
}

/// Root expressions of the model that are not part of an action.
pub fn model_roots(model: &Model) -> Vec<(ExprId, ExprContext)> {
    use ExprContext::*;
    let mut roots = Vec::new();
    for goal in &model.goals {
        push_goal(&goal.goal_expression, false, &mut roots);
    }
    for pref in model.preferences.iter() {
        push_goal(&pref.goal.goal_expression, true, &mut roots);
    }
    for eff in &model.init {
        push_effect(eff, &mut roots);
    }
    for metric in &model.metrics {
        match metric {
            Metric::Minimize(e) | Metric::Maximize(e) => roots.push((*e, Other)),
        }
    }
    if let Some(tn) = &model.task_network {
        push_task_network(tn, &mut roots);
    }
    roots
}

/// Root expressions of an action.
pub fn action_roots(action: &Action) -> Vec<(ExprId, ExprContext)> {
    use ExprContext::*;
    let mut roots = Vec::new();
    match action.duration {
        Duration::Instantaneous | Duration::Subtasks => {}
        Duration::Fixed(e) => roots.push((e, Other)),
        Duration::Bounded(lb, ub) => roots.extend([(lb, Other), (ub, Other)]),
    }
    roots.extend(action.achieved_task.args.iter().map(|&e| (e, Other)));
    roots.extend(action.conditions.iter().map(|c| (c.cond, Requirement)));
    roots.extend(action.preferences.iter().map(|p| (p.goal.cond, Other)));
    for eff in &action.effects {
        push_effect(eff, &mut roots);
    }
    push_task_network(&action.subtasks, &mut roots);
    roots
}

fn push_goal(goal: &SimpleGoal, is_preference: bool, roots: &mut Vec<(ExprId, ExprContext)>) {
    let ctx = if is_preference {
        ExprContext::Other
    } else {
        ExprContext::Requirement
    };
    match *goal {
        SimpleGoal::HoldsDuring(_, e) | SimpleGoal::SometimeDuring(_, e) => roots.push((e, ctx)),
        // the expression may be required to be false at some point
        SimpleGoal::AtMostOnceDuring(_, e) => roots.push((e, ExprContext::Other)),
        // the `when` part is an antecedent (thus with a negative polarity)
        SimpleGoal::SometimeBefore { when, then }
        | SimpleGoal::SometimeAfter { when, then }
        | SimpleGoal::AlwaysWithin { when, then, .. } => {
            roots.extend([(when, ExprContext::Other), (then, ctx)]);
        }
    }
}

fn push_effect(eff: &Effect, roots: &mut Vec<(ExprId, ExprContext)>) {
    let eff = &eff.effect_expression;
    roots.extend(eff.condition.iter().map(|&c| (c, ExprContext::Other)));
    roots.extend(eff.state_variable.arguments.iter().map(|&a| (a, ExprContext::Other)));
    match eff.operation {
        EffectOp::Assign(e) | EffectOp::Increase(e) | EffectOp::Decrease(e) => roots.push((e, ExprContext::Other)),
        EffectOp::Erase => {}
    }
}

fn push_task_network(tn: &TaskNet, roots: &mut Vec<(ExprId, ExprContext)>) {
    roots.extend(tn.constraints.iter().map(|&c| (c, ExprContext::Requirement)));
    for (_, subtask) in tn.iter() {
        roots.extend(subtask.args.iter().map(|&a| (a, ExprContext::Other)));
    }
}

/// Returns the direct sub-expressions of an expression.
pub fn children(expr: ExprId, env: &Environment) -> SeqExprId {
    match env.node(expr).expr() {
        Expr::App(_, args) | Expr::StateVariable(_, args) => args.clone(),
        Expr::Exists(_, e) | Expr::Forall(_, e) => smallvec::smallvec![*e],
        Expr::Real(_)
        | Expr::Bool(_)
        | Expr::Object(_)
        | Expr::Param(_)
        | Expr::Instant(_)
        | Expr::Duration
        | Expr::Makespan
        | Expr::PlanLength
        | Expr::ViolationCount(_) => SeqExprId::new(),
    }
}

/// Decomposes an expression into the list of its conjuncts (recursively flattening nested conjunctions).
pub fn conjuncts(expr: ExprId, env: &Environment) -> Vec<ExprId> {
    let mut res = Vec::new();
    let mut stack = vec![expr];
    while let Some(e) = stack.pop() {
        match env.node(e).expr() {
            Expr::App(Fun::And, args) => stack.extend(args.iter().rev()),
            _ => res.push(e),
        }
    }
    res
}

/// Returns all state variable expressions appearing in the expression (including itself).
pub fn state_variables(expr: ExprId, env: &Environment) -> Vec<ExprId> {
    let mut res = Vec::new();
    let mut stack = vec![expr];
    while let Some(e) = stack.pop() {
        if matches!(env.node(e).expr(), Expr::StateVariable(..)) {
            res.push(e);
        }
        stack.extend(children(e, env));
    }
    res
}
