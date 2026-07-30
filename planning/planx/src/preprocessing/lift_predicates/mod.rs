mod substitutions;

use itertools::Itertools;
use substitutions::*;

use std::collections::{HashMap, HashSet};

use crate::errors::EnvError;
use crate::{
    Action, Duration, Effect, EffectOp, Environment, Expr, ExprId, FluentId, Fun, Model, Object, Res, SeqExprId,
    SimpleGoal, Sym, Type,
};

/// Substitute predicates into state functions where applicable.
///
/// For instance the predicate `(at agent location) -> boolean` can usually be
/// transformed into the state function `(at agent) -> location`.
/// For this transformation to be applicable, it should be the case that,
/// for a given `agent`, there is at most one `location` such that `(at agent location) = true`.
///
/// The process is inspired by the paper:
/// "Extracting Mutual Exclusion Invariants from Lifted Temporal Planning Domains"
pub fn lift_predicates_to_state_functions(model: &mut Model) -> Res<()> {
    // Identifies a set of candidate groups whose predicates may be substituted with a state function.
    let mut candidates = collect_candidate_substitution_groups(model);

    // Sort the candidates by decreasing length.
    // This gives higher priority to groups that contains more predicates.
    // This is a heuristic choice, as they are not necessarily better.
    //
    // An important side effect is that group corresponding to static predicates,
    // which contain at most one predicate, are processed last.
    // This is critical (!) as processing groups of size 2 might need them in
    // their current form, before they are processed themselves.
    candidates.sort_by_key(|group| group.substitutions.len());
    candidates.reverse();

    // The same predicate may appear multiple times in different groups.
    // Only keep a predicate in the first group it is found to appear in (and remove it from the others).
    let mut to_remove = HashSet::new();
    let mut i = 0;
    while i < candidates.len() {
        let group = &candidates[i];

        if group
            .substitutions
            .iter()
            .any(|sub| to_remove.contains(&sub.predicate_id))
        {
            candidates.remove(i);
        } else {
            to_remove.extend(group.substitutions.iter().map(|sub| sub.predicate_id));
            i += 1;
        }
    }

    if !candidates.is_empty() {
        println!("Lifting predicates to state functions:")
    }
    for group in &candidates {
        println!("  - {group:<40} [from: {group:?}]");
        lift(model, group)?;
    }
    Ok(())
}

/// Transform the expressions over the group's predicates to use one new state function instead.
fn lift(model: &mut Model, group: &SubstitutionGroup) -> Res<()> {
    debug_assert!(group.is_substitutable(model));

    // Apply the substitution group (and delete the group's predicates and register the substituting state function).
    let group = AppliedSubstitutionGroup::new(group, &mut model.env)?;

    // Outside actions (global/top level)
    {
        for expr_id in Vec::from_iter(iter_global_noneffect_exprs(model)) {
            transform_noneffect_exprs_recursive(expr_id, &group, &mut model.env)?;
        }

        let (model_env, effects) = get_mut_global_effect_exprs(model);
        transform_effect_exprs(effects, &group, model_env, None)?;
    }

    // In actions
    for name in Vec::from_iter(model.actions.iter().map(|act| act.name.clone())) {
        let act = model.actions.get_action_mut(&name).unwrap();

        for &expr_id in iter_action_noneffect_exprs(act) {
            transform_noneffect_exprs_recursive(expr_id, &group, &mut model.env)?;
        }
        let d = act.duration.clone();
        transform_effect_exprs(get_mut_action_effect_exprs(act), &group, &mut model.env, Some(d))?;
    }

    Ok(())
}

/// Substitution group together with the id of the newly introduced state function to substitute it.
///
/// Stores the return type of the fluent, allowing direct access without needing to get the model's environment.
/// Also, when the return type is a new, created helper type, stores the created synthetic / helper objects of that type.
/// There is one such helper object per predicate in the underlying group (see `SubstitutionGroupReturnType`).
#[derive(Debug)]
struct AppliedSubstitutionGroup<'a> {
    group: &'a SubstitutionGroup,
    substitution_fluent_id: FluentId,
    // return_type: Type,
    helper_objects: Option<Vec<Object>>,
}

impl<'a> AppliedSubstitutionGroup<'a> {
    /// NOTE: deletes the group's predicates and adds the substituting state function
    fn new(group: &'a SubstitutionGroup, env: &mut Environment) -> Res<Self> {
        let (return_type, helper_objects) = match &group.return_type {
            SubstitutionGroupReturnType::KnownType(return_type) => {
                debug_assert!(matches!(return_type, Type::User(_)));
                (return_type.clone(), None)
            }
            SubstitutionGroupReturnType::NewHelperType => {
                let mut helper_objects = vec![];

                let tpe_name = ["_help-tpe-", group.to_string().as_str()].join("");

                env.types.add_top_type_child(tpe_name.as_str())?;
                let tpe = env.types.get_union_user_type(tpe_name.as_str()).msg(env)?;

                for sub in &group.substitutions {
                    let obj_name = Sym::from(
                        ["_help-obj-", env.fluents.get(sub.predicate_id).name().as_str()]
                            .join("")
                            .as_str(),
                    );

                    env.objects.add_object(&obj_name, tpe.to_single_type().unwrap())?;

                    helper_objects.push(env.objects.get(obj_name)?);
                }
                (Type::User(tpe), Some(helper_objects))
            }
        };

        env.fluents.remove(|fluent_id, _| group.contains(fluent_id));

        let substitution_fluent_id = env
            .fluents
            .add_fluent(
                group.to_string().as_str(),
                group.params.clone(),
                return_type.clone(),
                None,
            )
            .msg(env)?;

        debug_assert!(
            helper_objects
                .as_ref()
                .is_none_or(|v| v.len() == group.substitutions.len())
        );

        Ok(Self {
            group,
            substitution_fluent_id,
            // return_type,
            helper_objects,
        })
    }

    fn helper_object(&self, predicate_id: FluentId) -> Option<&Object> {
        assert!(self.helper_objects.is_some());
        self.helper_objects.as_ref().unwrap().get(
            self.group
                .substitutions
                .iter()
                .position(|sub| sub.predicate_id == predicate_id)?,
        )
    }

    fn get_lifted_param_idx(&self, predicate_id: FluentId) -> Option<usize> {
        get_lifted_param_idx(self.group, predicate_id)
    }
}

fn get_lifted_param_idx(group: &SubstitutionGroup, predicate_id: FluentId) -> Option<usize> {
    let mut ii = group
        .substitutions
        .iter()
        .filter(|sub| sub.predicate_id == predicate_id && sub.return_param_idx.is_some())
        .flat_map(|sub| sub.return_param_idx);
    let i = ii.next();
    debug_assert!(ii.next().is_none());
    i
}

/// Recursively visits the expressions under the given one,
/// and transforms predicate expressions over the group's predicates to use the substitution state function.
///
/// The transformations are: `(at x y)` -> `(= (at x) y)` and `(not (at x y))` -> `(not (= (at x) y))`)
/// where `y` is indicated by the (index of the) lifted return parameter of the group.
///
/// When none of the group's predicates have a lifted return parameter (i.e. the group's return type is a new helper type),
/// the transformations for a group composed of `pred1` and `pred2` are:
/// `(pred1)` -> `(= (pred1) _help-obj-pred1)`, `(not (pred1))` -> `(not (= (pred1) _help-obj-pred1))`,
/// `(pred2)` -> `(= (pred2) _help-obj-pred1)`, and `(not (pred2))` -> `(not (= (pred2) _help-obj-pred2))`,
fn transform_noneffect_exprs_recursive(
    expr_id: ExprId,
    group: &AppliedSubstitutionGroup,
    env: &mut Environment,
) -> Res<()> {
    let aux_closure = |eid: ExprId, predicate_id: FluentId, args: SeqExprId, env: &mut Environment| -> Res<()> {
        let lifted_param_idx = group.get_lifted_param_idx(predicate_id);

        let (val_expr, new_sv_expr) = if let Some(lifted_param_idx) = lifted_param_idx {
            debug_assert!(
                matches!(group.group.return_type, SubstitutionGroupReturnType::KnownType(_))
                    && group.helper_objects.is_none()
            );

            let mut new_args = args;
            let val_expr = new_args.remove(lifted_param_idx);
            let new_sv_expr = env.intern(Expr::StateVariable(group.substitution_fluent_id, new_args), None)?;
            (val_expr, new_sv_expr)
        } else {
            debug_assert!(
                matches!(group.group.return_type, SubstitutionGroupReturnType::NewHelperType)
                    && group.helper_objects.as_ref().is_some_and(|v| !v.is_empty())
            );

            let val_expr = env.intern(Expr::Object(group.helper_object(predicate_id).unwrap().clone()), None)?;
            let new_sv_expr = env.intern(Expr::StateVariable(group.substitution_fluent_id, args), None)?;
            (val_expr, new_sv_expr)
        };

        env.replace(
            eid,
            Expr::App(Fun::Eq, [new_sv_expr, val_expr].into_iter().collect()),
            None,
        )?;

        Ok(())
    };

    let mut closure = |expr_id: ExprId, env: &mut Environment| {
        match try_into_predicate_expr(expr_id, env) {
            Some(PredicateExpr::Positive(eid, predicate_id, args)) if group.group.contains(predicate_id) => {
                aux_closure(eid, predicate_id, args, env)?;
            }
            Some(PredicateExpr::Negative(_, _, predicate_id, _)) if group.group.contains(predicate_id) => {
                unreachable!("groups with negative conditions on its predicates are deemed unsubstitutable");
            }
            _ => (),
        };
        Ok(())
    };

    visit_exprs_recursive_and_apply_mut(expr_id, &mut closure, env)
}

/// Transform effect expressions over predicates included in the group.
///
/// Assumes the group is indeed substitutable, notably meaning that for each fluent in it:
/// - in each action, there's exactly one positive and one negative effect on it (and no negative conditions)
/// - outside of actions, there's at most one positive effect on it and no negative effects (and no negative conditions)
///
/// Transformations are similar to those for conditions/constraints (see `transform_noneffect_exprs_recursive`),
/// but negative effects happening at the same time as a positive one (with the same args) are deleted.
fn transform_effect_exprs(
    effects: &mut Vec<Effect>,
    group: &AppliedSubstitutionGroup,
    env: &mut Environment,
    container_duration: Option<Duration>,
) -> Res<()> {
    let try_into_simple_args = |predicate_id: FluentId, args: &[ExprId]| -> Option<Vec<SimpleArg>> {
        debug_assert!(group.group.contains(predicate_id));
        let sub_idx = group
            .group
            .substitutions
            .iter()
            .position(|sub| sub.predicate_id == predicate_id)
            .unwrap();

        group.group.reorderings[sub_idx]
            .permutation
            .iter()
            .map(|&i| match env.node(args[i]).expr() {
                Expr::Real(x) => Some(SimpleArg::Cst(CstArg::Real(*x))),
                Expr::Bool(x) => Some(SimpleArg::Cst(CstArg::Bool(*x))),
                Expr::Object(x) => Some(SimpleArg::Cst(CstArg::Object(x.name().clone()))),
                Expr::Param(x) => Some(SimpleArg::Param(x.name().clone())),
                _ => None,
            })
            .collect::<Option<_>>()
    };

    let mut pos_effects = HashMap::new();
    let mut neg_effects = HashMap::new();

    for (i, eff) in effects.iter_mut().enumerate() {
        let eff = &mut eff.effect_expression;

        if !group.group.contains(eff.state_variable.fluent) {
            continue;
        }

        if let EffectOp::Assign(eid) = eff.operation {
            match env.node(eid).expr() {
                Expr::Bool(true) => {
                    pos_effects
                        .insert(
                            try_into_simple_args(eff.state_variable.fluent, &eff.state_variable.arguments).unwrap(),
                            (i, eff.state_variable.fluent),
                        )
                        .inspect(|_| unreachable!());
                }
                Expr::Bool(false) => {
                    neg_effects
                        .insert(
                            try_into_simple_args(eff.state_variable.fluent, &eff.state_variable.arguments).unwrap(),
                            (i, eff.state_variable.fluent),
                        )
                        .inspect(|_| unreachable!());
                }
                _ => (),
            }
        } else if let EffectOp::Erase = eff.operation {
            neg_effects
                .insert(
                    try_into_simple_args(eff.state_variable.fluent, &eff.state_variable.arguments).unwrap(),
                    (i, eff.state_variable.fluent),
                )
                .inspect(|_| unreachable!());
        }
    }

    let container_duration_bounds = if let Some(d) = container_duration.as_ref() {
        get_duration_lower_and_upper_bounds(d, env)
    } else {
        (0.into(), None)
    };

    let mut neg_effects_to_null = vec![];
    let mut neg_effects_to_del = vec![];
    let mut neg_effects_to_keep = vec![];

    for (k, (i, _)) in neg_effects {
        if let Some(&(j, _)) = pos_effects.get(&k) {
            let Some(delay) = get_timings_delay_lower_bound(
                effects[i].effect_expression.timing,
                effects[j].effect_expression.timing,
                container_duration_bounds,
            ) else {
                unreachable!()
            };
            // (t1 necessarily <= t2)
            debug_assert!(delay >= 0.into());

            // (t1 necessarily < t2)
            if delay == 0.into() {
                neg_effects_to_del.push(i);
            } else if matches!(effects[i].effect_expression.operation, EffectOp::Erase) {
                neg_effects_to_keep.push(i);
            } else {
                neg_effects_to_null.push(i);
            }
        } else {
            if matches!(effects[i].effect_expression.operation, EffectOp::Erase) {
                neg_effects_to_keep.push(i);
            } else {
                neg_effects_to_null.push(i);
            }
        }
    }

    for (idx, _) in pos_effects.into_values() {
        let eff = &mut effects[idx].effect_expression;
        debug_assert!(group.group.contains(eff.state_variable.fluent));

        let lifted_param_idx = group.get_lifted_param_idx(eff.state_variable.fluent);

        if let Some(lifted_param_idx) = lifted_param_idx {
            if !matches!(eff.operation, EffectOp::Erase) {
                eff.operation = EffectOp::Assign(eff.state_variable.arguments[lifted_param_idx]);
            }
            eff.state_variable.arguments.remove(lifted_param_idx);
        } else {
            if !matches!(eff.operation, EffectOp::Erase) {
                eff.operation = EffectOp::Assign(env.intern(
                    Expr::Object(group.helper_object(eff.state_variable.fluent).unwrap().clone()),
                    None,
                )?);
            }
        }
        eff.state_variable.fluent = group.substitution_fluent_id;
    }

    for idx in neg_effects_to_keep {
        let eff = &mut effects[idx].effect_expression;
        debug_assert!(group.group.contains(eff.state_variable.fluent));

        if let Some(lifted_param_idx) = group.get_lifted_param_idx(eff.state_variable.fluent) {
            eff.state_variable.arguments.remove(lifted_param_idx);
        }
        eff.state_variable.fluent = group.substitution_fluent_id;
    }

    for idx in neg_effects_to_null {
        let eff = &mut effects[idx].effect_expression;
        debug_assert!(group.group.contains(eff.state_variable.fluent));

        eff.operation = EffectOp::Erase;

        if let Some(lifted_param_idx) = group.get_lifted_param_idx(eff.state_variable.fluent) {
            eff.state_variable.arguments.remove(lifted_param_idx);
        }
        eff.state_variable.fluent = group.substitution_fluent_id;
    }

    for idx in neg_effects_to_del.into_iter().sorted().rev() {
        effects.remove(idx);
    }
    Ok(())
}

enum PredicateExpr {
    /// Positive predicate expression, stating the predicate must hold. (e.g. `(at x y)`).
    Positive(ExprId, FluentId, SeqExprId),
    /// Negative predicate expression, stating the predicate must not hold. (e.g. `(not (at x y)`).
    /// The first id is that of the `not` function application, and the second one is that of its contents (e.g. `(at x y)`).
    #[allow(unused)] // First parameter is currently unused
    Negative(ExprId, ExprId, FluentId, SeqExprId),
}

/// Converts the given expression to a view of a (boolean) predicate expression, if it corresponds to one.
///
/// Note: On call, the `fluent_id` of matched StateVariable expressions in this function
/// may have already been deleted from `env.fluents` (as predicates of a (applied) substitution group).
fn try_into_predicate_expr(expr_id: ExprId, env: &Environment) -> Option<PredicateExpr> {
    if let Expr::App(Fun::Not, inner) = env.node(expr_id).expr()
        && inner.len() == 1
        && let Expr::StateVariable(fluent_id, args) = env.node(inner[0]).expr()
    {
        return Some(PredicateExpr::Negative(expr_id, inner[0], *fluent_id, args.clone()));
    } else if let Expr::StateVariable(fluent_id, args) = env.node(expr_id).expr() {
        return Some(PredicateExpr::Positive(expr_id, *fluent_id, args.clone()));
    }
    None
}

fn iter_global_preferences_exprs(model: &Model) -> impl IntoIterator<Item = ExprId> {
    let mut res = vec![];

    for goal in model.preferences.iter().map(|pref| &pref.goal) {
        match goal.goal_expression {
            SimpleGoal::HoldsDuring(_time_interval, expr_id) => res.push(expr_id),
            SimpleGoal::SometimeDuring(_time_interval, expr_id) => res.push(expr_id),
            SimpleGoal::AtMostOnceDuring(_time_interval, expr_id) => res.push(expr_id),
            SimpleGoal::SometimeBefore { when, then } => {
                res.push(when);
                res.push(then)
            }
            SimpleGoal::SometimeAfter { when, then } => {
                res.push(when);
                res.push(then)
            }
            SimpleGoal::AlwaysWithin {
                delay: _delay,
                when,
                then,
            } => {
                res.push(when);
                res.push(then)
            }
        }
    }
    res
}

fn iter_global_noneffect_exprs(model: &Model) -> impl IntoIterator<Item = ExprId> {
    let mut res = vec![];

    for goal in model.goals.iter() {
        match goal.goal_expression {
            SimpleGoal::HoldsDuring(_time_interval, expr_id) => res.push(expr_id),
            SimpleGoal::SometimeDuring(_time_interval, expr_id) => res.push(expr_id),
            SimpleGoal::AtMostOnceDuring(_time_interval, expr_id) => res.push(expr_id),
            SimpleGoal::SometimeBefore { when, then } => {
                res.push(when);
                res.push(then)
            }
            SimpleGoal::SometimeAfter { when, then } => {
                res.push(when);
                res.push(then)
            }
            SimpleGoal::AlwaysWithin {
                delay: _delay,
                when,
                then,
            } => {
                res.push(when);
                res.push(then)
            }
        }
    }
    res.extend(iter_global_preferences_exprs(model));
    res.extend(
        get_global_effect_exprs(model)
            .iter()
            .flat_map(|eff| &eff.effect_expression.condition),
    );
    res.extend(model.task_network.iter().flat_map(|tn| tn.constraints.iter().copied()));
    res
}

fn iter_action_preferences_exprs(action: &Action) -> impl Iterator<Item = &ExprId> {
    action.preferences.iter().map(|pref| &pref.goal.cond)
}

fn iter_action_noneffect_exprs(action: &Action) -> impl Iterator<Item = &ExprId> {
    let res = action
        .conditions
        .iter()
        .map(|cond| &cond.cond)
        .chain(iter_action_preferences_exprs(action))
        .chain(action.effects.iter().flat_map(|eff| &eff.effect_expression.condition));

    res.chain(action.subtasks.constraints.iter())
}

fn get_global_effect_exprs(model: &Model) -> &[Effect] {
    &model.init
}
fn get_mut_global_effect_exprs(model: &mut Model) -> (&mut Environment, &mut Vec<Effect>) {
    (&mut model.env, &mut model.init)
}
fn get_action_effect_exprs(action: &Action) -> &[Effect] {
    &action.effects
}
fn get_mut_action_effect_exprs(action: &mut Action) -> &mut Vec<Effect> {
    &mut action.effects
}

fn visit_exprs_recursive_and_apply_mut<F>(expr_id: ExprId, func: &mut F, env: &mut Environment) -> Res<()>
where
    F: FnMut(ExprId, &mut Environment) -> Res<()>,
{
    func(expr_id, env)?;

    match env.node(expr_id).expr() {
        Expr::App(_, eids) | Expr::StateVariable(_, eids) => {
            for eid in eids.clone() {
                visit_exprs_recursive_and_apply_mut(eid, func, env)?;
            }
        }
        Expr::Exists(_, eid) => visit_exprs_recursive_and_apply_mut(*eid, func, env)?,
        Expr::Forall(_, eid) => visit_exprs_recursive_and_apply_mut(*eid, func, env)?,
        Expr::Real(_)
        | Expr::Bool(_)
        | Expr::Object(_)
        | Expr::Param(_)
        | Expr::Instant(_)
        | Expr::Duration
        | Expr::Makespan
        | Expr::ViolationCount(_) => (),
    }
    Ok(())
}

fn visit_exprs_recursive_and_apply<F>(expr_id: ExprId, func: &mut F, env: &Environment) -> Res<()>
where
    F: FnMut(ExprId, &Environment) -> Res<()>,
{
    func(expr_id, env)?;

    match env.node(expr_id).expr() {
        Expr::App(_, eids) | Expr::StateVariable(_, eids) => {
            for &eid in eids {
                visit_exprs_recursive_and_apply(eid, func, env)?;
            }
        }
        Expr::Exists(_, eid) => visit_exprs_recursive_and_apply(*eid, func, env)?,
        Expr::Forall(_, eid) => visit_exprs_recursive_and_apply(*eid, func, env)?,
        Expr::Real(_)
        | Expr::Bool(_)
        | Expr::Object(_)
        | Expr::Param(_)
        | Expr::Instant(_)
        | Expr::Duration
        | Expr::Makespan
        | Expr::ViolationCount(_) => (),
    }
    Ok(())
}

fn visit_exprs_recursive_and_check<F>(expr_id: ExprId, func: &F, env: &Environment) -> Res<()>
where
    F: Fn(ExprId, &Environment) -> Res<()>,
{
    func(expr_id, env)?;

    match env.node(expr_id).expr() {
        Expr::App(_, eids) | Expr::StateVariable(_, eids) => {
            for &eid in eids {
                visit_exprs_recursive_and_check(eid, func, env)?;
            }
        }
        Expr::Exists(_, eid) => visit_exprs_recursive_and_check(*eid, func, env)?,
        Expr::Forall(_, eid) => visit_exprs_recursive_and_check(*eid, func, env)?,
        Expr::Real(_)
        | Expr::Bool(_)
        | Expr::Object(_)
        | Expr::Param(_)
        | Expr::Instant(_)
        | Expr::Duration
        | Expr::Makespan
        | Expr::ViolationCount(_) => (),
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{Message, Model, Res, pddl::*};
    use std::path::{Path, PathBuf};

    fn parse_pddl(domain_file: &Path, problem_file: &Path) -> Res<(Model, Model)> {
        let domain_file = input::Input::from_file(domain_file)?;

        let problem_file = input::Input::from_file(problem_file)?;
        let domain = parser::parse_pddl_domain(domain_file)?;
        let problem = parser::parse_pddl_problem(problem_file)?;

        let nonlifted_model = build_model(&domain, &problem)?;
        let lifted_model = {
            let mut res = build_model(&domain, &problem)?;
            super::lift_predicates_to_state_functions(&mut res)?;
            res
        };
        Ok((nonlifted_model, lifted_model))
    }

    fn get_fluent_by_name<'a>(model: &'a Model, fluent_name: &'a str) -> Res<&'a crate::Fluent> {
        Ok(model.env.fluents.get(
            model
                .env
                .fluents
                .get_by_name(fluent_name)
                .ok_or(Message::error("unknown fluent name"))?,
        ))
    }

    fn simple_test(
        domain_file: &Path,
        problem_file: &Path,
        expected_lifted_fluents: usize,
        expected_lifted_fluents_with_helper_types: usize,
        expected_lifted_fluents_shapes: &[(&str, usize, &str)],
    ) -> Res<(Model, Model)> {
        let (nonlifted_model, lifted_model) = parse_pddl(domain_file, problem_file)?;

        println!("== BEFORE LIFTING PREDICATES ==");
        println!("{nonlifted_model}");
        println!("== AFTER LIFTING PREDICATES ==");
        print!("{lifted_model}");

        assert!(
            nonlifted_model
                .env
                .fluents
                .iter()
                .filter(|fluent| matches!(&fluent.return_type, crate::Type::User(_)))
                .count()
                == 0
        );

        assert!(
            lifted_model
                .env
                .fluents
                .iter()
                .filter(|fluent| matches!(&fluent.return_type, crate::Type::User(_)))
                .count()
                == expected_lifted_fluents
        );
        assert!(
            lifted_model
                .env
                .fluents
                .iter()
                .filter(|fluent| matches!(
                    &fluent.return_type, crate::Type::User(tpe)
                    if tpe.to_single_type().unwrap().name.as_str().starts_with("_help-tpe-")
                ))
                .count()
                == expected_lifted_fluents_with_helper_types
        );

        for &(fluent_name, expected_num_params, expected_return_type_name) in expected_lifted_fluents_shapes {
            let fluent = get_fluent_by_name(&lifted_model, fluent_name)?;
            assert!(fluent.parameters.len() == expected_num_params, "{fluent_name:?}");
            assert!(
                matches!(
                    &fluent.return_type, crate::Type::User(user_type)
                    if user_type.members() == [expected_return_type_name]
                ),
                "{fluent_name:?}",
            );
        }

        Ok((nonlifted_model, lifted_model))
    }

    #[test]
    fn test_gripper() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/pddl/ipc/1998-gripper-round-1-strips/domain.pddl"),
            &PathBuf::from("../problems/pddl/ipc/1998-gripper-round-1-strips/instance.1.pb.pddl"),
            2,
            0,
            &[("at-robby", 0, "object"), ("carry:at", 1, "object")],
        )?;
        Ok(())
    }

    #[test]
    fn test_satellite_strips() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-satellite-strips-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-satellite-strips-automatic/problem.pddl"),
            4,
            0,
            &[
                ("calibration_target", 1, "direction"),
                ("pointing", 1, "direction"),
                ("supports", 1, "mode"),
                ("on_board", 1, "satellite"),
            ],
        )?;
        Ok(())
    }

    #[test]
    fn test_satellite_time() -> Res<()> {
        let (_, lifted_model) = simple_test(
            &PathBuf::from("../problems/upf/ipc2002-satellite-time-simple-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-satellite-time-simple-automatic/problem.pddl"),
            4,
            0,
            &[
                ("calibration_target", 1, "direction"),
                ("pointing", 1, "direction"),
                ("supports", 1, "mode"),
                ("on_board", 1, "satellite"),
            ],
        )?;

        let turn_to = lifted_model.actions.get_action(&crate::Sym::from("turn_to")).unwrap();
        assert!(turn_to.effects.len() == 2);

        let eff_pointing_end = &turn_to.effects[0].effect_expression;
        let eff_pointing_start = &turn_to.effects[1].effect_expression;

        assert!(eff_pointing_start.timing.reference == crate::TimeRef::ActionStart);
        assert!(
            lifted_model
                .env
                .fluents
                .get(eff_pointing_start.state_variable.fluent)
                .name()
                .as_str()
                == "pointing"
        );
        assert!(matches!(eff_pointing_start.operation, crate::EffectOp::Erase));

        assert!(eff_pointing_end.timing.reference == crate::TimeRef::ActionEnd);
        assert!(
            lifted_model
                .env
                .fluents
                .get(eff_pointing_end.state_variable.fluent)
                .name()
                .as_str()
                == "pointing"
        );
        assert!(matches!(eff_pointing_end.operation, crate::EffectOp::Assign(_)));

        Ok(())
    }

    #[test]
    fn test_psr() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2004-psr-small-strips/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2004-psr-small-strips/problem.pddl"),
            5,
            5,
            &[
                (
                    "do_normal:do_wait_cb1_condeffs:do_close_sd1_condeffs",
                    0,
                    "_help-tpe-do_normal:do_wait_cb1_condeffs:do_close_sd1_condeffs",
                ),
                (
                    "not_updated_cb1:updated_cb1",
                    0,
                    "_help-tpe-not_updated_cb1:updated_cb1",
                ),
                ("closed_sd1:not_closed_sd1", 0, "_help-tpe-closed_sd1:not_closed_sd1"),
                ("closed_sd2:not_closed_sd2", 0, "_help-tpe-closed_sd2:not_closed_sd2"),
                ("closed_cb1:not_closed_cb1", 0, "_help-tpe-closed_cb1:not_closed_cb1"),
            ],
        )?;
        Ok(())
    }

    #[test]
    fn test_rovers_strips() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-rovers-strips-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-rovers-strips-automatic/problem.pddl"),
            13,
            1,
            &[
                ("full:empty", 1, "_help-tpe-full:empty"),
                ("channel_free", 0, "lander"),
                ("on_board", 1, "rover"),
                ("calibration_target", 1, "objective"),
                ("store_of", 1, "rover"),
                ("available", 0, "rover"),
                ("supports", 1, "camera"),
                ("equipped_for_imaging", 0, "rover"),
                ("equipped_for_rock_analysis", 0, "rover"),
                ("equipped_for_soil_analysis", 0, "rover"),
                ("can_traverse", 2, "rover"),
                ("at_lander", 1, "waypoint"),
                ("at_", 1, "waypoint"),
            ],
        )?;
        Ok(())
    }

    #[test]
    fn test_rovers_time() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-rovers-time-simple-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-rovers-time-simple-automatic/problem.pddl"),
            13,
            1,
            &[
                ("full:empty", 1, "_help-tpe-full:empty"),
                ("channel_free", 0, "lander"),
                ("on_board", 1, "rover"),
                ("calibration_target", 1, "objective"),
                ("store_of", 1, "rover"),
                ("available", 0, "rover"),
                ("supports", 1, "camera"),
                ("equipped_for_imaging", 0, "rover"),
                ("equipped_for_rock_analysis", 0, "rover"),
                ("equipped_for_soil_analysis", 0, "rover"),
                ("can_traverse", 2, "rover"),
                ("at_lander", 1, "waypoint"),
                ("at_", 1, "waypoint"),
            ],
        )?;
        Ok(())
    }

    #[test]
    fn test_rovers_numeric() -> Res<()> {
        simple_test(
            &PathBuf::from("../problems/upf/ipc2002-rovers-numeric-automatic/domain.pddl"),
            &PathBuf::from("../problems/upf/ipc2002-rovers-numeric-automatic/problem.pddl"),
            14,
            1,
            &[
                ("full:empty", 1, "_help-tpe-full:empty"),
                ("in_sun", 0, "waypoint"),
                ("channel_free", 0, "lander"),
                ("on_board", 1, "rover"),
                ("calibration_target", 1, "objective"),
                ("store_of", 1, "rover"),
                ("available", 0, "rover"),
                ("supports", 1, "camera"),
                ("equipped_for_imaging", 0, "rover"),
                ("equipped_for_rock_analysis", 0, "rover"),
                ("equipped_for_soil_analysis", 0, "rover"),
                ("can_traverse", 2, "rover"),
                ("at_lander", 1, "waypoint"),
                ("at_", 1, "waypoint"),
            ],
        )?;
        Ok(())
    }
}
