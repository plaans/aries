mod substitutions;

use aries_env_param::EnvParam;
use itertools::Itertools;
use substitutions::*;

use std::collections::{HashMap, HashSet};

use crate::errors::EnvError;
use crate::{
    Action, Effect, EffectOp, Environment, Expr, ExprId, FluentId, Fun, Model, Object, Res, SeqExprId, SimpleGoal, Sym,
    Type, UnionUserType,
};

pub static ARIES_LIFT_PREDICATES: EnvParam<bool> = EnvParam::new("ARIES_LIFT_PREDICATES", "false");

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
        transform_effect_exprs(effects, &group, model_env)?;
    }

    // In actions
    for name in Vec::from_iter(model.actions.iter().map(|act| act.name.clone())) {
        let act = model.actions.get_action_mut(&name).unwrap();

        for &expr_id in iter_action_noneffect_exprs(act) {
            transform_noneffect_exprs_recursive(expr_id, &group, &mut model.env)?;
        }
        transform_effect_exprs(get_mut_action_effect_exprs(act), &group, &mut model.env)?;
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
    pub fn new(group: &'a SubstitutionGroup, env: &mut Environment) -> Res<Self> {
        let (return_type, helper_objects) = match &group.return_type {
            SubstitutionGroupReturnType::KnownType(return_type) => {
                debug_assert!(matches!(return_type, Type::User(_)));
                (return_type.clone(), None)
            }
            SubstitutionGroupReturnType::NewHelperType => {
                let mut helper_objects = vec![];

                let tpe_name = ["_help-tpe-", group.to_string().as_str()].join("");
                let tpe = UnionUserType::new(tpe_name.as_str(), env.types.top_user_type().hier);

                env.types.add_user_type_independent(tpe.to_single_type().unwrap().name);

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

    pub fn helper_object(&self, predicate_id: FluentId) -> Option<&Object> {
        assert!(self.helper_objects.is_some());
        self.helper_objects.as_ref().unwrap().get(
            self.group
                .substitutions
                .iter()
                .position(|sub| sub.predicate_id == predicate_id)?,
        )
    }

    fn get_lifted_param_idx(&self, predicate_id: FluentId) -> Option<usize> {
        get_lifted_param_idx(&self.group, predicate_id)
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
            Some(PredicateExpr::Negative(_, inner_eid, predicate_id, args)) if group.group.contains(predicate_id) => {
                // WARNING: POTENTIAL BUG (TODO FIXME):
                // Until "null"/"undefined" values/objects for user types are introduced, may theoretically result in unexpected behavior.
                // For example, suppose we are changing `(not (at r l))` into `(not (= (at r) l))`.
                // Indeed the semantics also technically change from "r is not at l" to "r is *somewhere other* than l".
                // With "null"/"undefined" values, the new semantics wouldn't break the old ones, as "r" would be allowed to take the "undefined" value.
                // Without such values, there is no guarantee that the behavior would be the same as without lifting, in the general case.
                // FIXME @arbimo correct ?
                aux_closure(inner_eid, predicate_id, args, env)?;
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
                Expr::Bool(false) => {
                    neg_effects.insert(
                        try_into_simple_args(eff.state_variable.fluent, &eff.state_variable.arguments).unwrap(),
                        (i, eff.state_variable.fluent),
                    );
                }
                Expr::Bool(true) => {
                    pos_effects.insert(
                        try_into_simple_args(eff.state_variable.fluent, &eff.state_variable.arguments).unwrap(),
                        (i, eff.state_variable.fluent),
                    );
                }
                _ => (),
            }
        }
    }

    let mut neg_effects_to_null = vec![];
    let mut neg_effects_to_del = vec![];

    for (k, &(i, _)) in &neg_effects {
        if let Some(&(j, _)) = pos_effects.get(k) {
            debug_assert!(timing_is_necessarily_before_or_eq(
                effects[i].effect_expression.timing,
                effects[j].effect_expression.timing,
            ));
            if effects[i].effect_expression.timing != effects[j].effect_expression.timing {
                // positive effect is necessarily strictly after the negative one (can happen in durative actions FIXME @arbimo right ?).
                debug_assert!(!timing_is_necessarily_before_or_eq(
                    effects[j].effect_expression.timing,
                    effects[i].effect_expression.timing,
                ));
                neg_effects_to_null.push(i);
            } else {
                neg_effects_to_del.push(i);
            }
        }
    }

    let pos_and_neg_effs = pos_effects.into_values().chain(neg_effects.into_values());
    for (idx, _) in pos_and_neg_effs {
        let eff = &mut effects[idx].effect_expression;
        debug_assert!(group.group.contains(eff.state_variable.fluent));

        let lifted_param_idx = group.get_lifted_param_idx(eff.state_variable.fluent);

        if let Some(lifted_param_idx) = lifted_param_idx {
            eff.operation = EffectOp::Assign(eff.state_variable.arguments[lifted_param_idx]);
            eff.state_variable.arguments.remove(lifted_param_idx);
        } else {
            eff.operation = EffectOp::Assign(env.intern(
                Expr::Object(group.helper_object(eff.state_variable.fluent).unwrap().clone()),
                None,
            )?);
        }
        eff.state_variable.fluent = group.substitution_fluent_id;
    }

    for idx in neg_effects_to_null {
        let eff = &mut effects[idx].effect_expression;
        debug_assert!(group.group.contains(eff.state_variable.fluent));
 
        let _lifted_param_idx = group.get_lifted_param_idx(eff.state_variable.fluent);

        todo!("TODO requires a 'null'/'undefined' objects for all user types TODO");
        // eff.operation = EffectOp::Assign(env.intern(Expr::Object(TODO_NULL_OBJECT), None)?);
        // if let Some(lifted_param_idx) = _lifted_param_idx {
        //     eff.state_variable.arguments.remove(lifted_param_idx);
        // }
        // eff.state_variable.fluent = group.substitution_fluent_id;
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
    #[allow(unused)]
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

fn iter_global_noneffect_exprs(model: &Model) -> impl IntoIterator<Item = ExprId> {
    let mut res = vec![];

    for goal in model
        .goals
        .iter()
        .chain(model.preferences.iter().map(|pref| &pref.goal))
    {
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
    res.extend(
        get_global_effect_exprs(model)
            .iter()
            .flat_map(|eff| &eff.effect_expression.condition),
    );
    res.extend(model.task_network.iter().flat_map(|tn| tn.constraints.iter().copied()));
    res
}

fn iter_action_noneffect_exprs(action: &Action) -> impl Iterator<Item = &ExprId> {
    let res = action
        .conditions
        .iter()
        .chain(action.preferences.iter().map(|pref| &pref.goal))
        .map(|cond| &cond.cond)
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
        _ => (),
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
        _ => (),
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
        _ => (),
    }
    Ok(())
}
