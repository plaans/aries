//! Rewriting of a model, replacing the predicates of a group by a single state function.

use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use smallvec::smallvec;

use super::groups::{ActionUpdates, Group, supertype};
use crate::errors::{EnvError, Spanned};
use crate::preprocessing::analysis::{Term, children, expression_roots};
use crate::*;

/// A group whose predicates have been replaced by a state function in the environment.
pub struct LiftedGroup {
    pub group: Group,
    /// The state function replacing the group's predicates.
    pub fluent: FluentId,
    /// For groups without value parameters: the value representing each member (same order as `group.members`).
    helpers: Vec<Object>,
}

impl LiftedGroup {
    /// Removes the group's predicates from the environment and declares the state function (and helper objects) replacing them.
    pub fn declare(group: Group, name: &str, env: &mut Environment) -> Res<Self> {
        let param = |m: usize, i: usize| env.fluents.get(group.members[m].fluent).parameters[i].clone();

        // parameters of the state function: for each key position, the parameter with the most general type
        let mut params = Vec::new();
        for k in 0..group.members[0].key.len() {
            let candidates = (0..group.members.len())
                .map(|m| param(m, group.members[m].key[k]))
                .collect_vec();
            let tpe = supertype(candidates.iter().map(|p| p.tpe().clone())).unwrap();
            params.push(candidates.into_iter().find(|p| p.tpe() == &tpe).unwrap());
        }

        let mut helpers = Vec::new();
        let value_type = if group.has_values() {
            supertype((0..group.members.len()).map(|m| param(m, group.members[m].value.unwrap()).tpe().clone()))
                .unwrap()
        } else {
            let tpe_name = format!("_help-tpe-{name}");
            env.types.add_top_type_child(tpe_name.as_str())?;
            let tpe = env.types.get_user_type(tpe_name.as_str()).msg(env)?;
            for m in &group.members {
                let obj_name = format!("_help-obj-{}", env.fluents.get(m.fluent).name());
                env.objects.add_object(obj_name.as_str(), tpe.clone()).msg(env)?;
                helpers.push(env.objects.get(obj_name.as_str()).msg(env)?);
            }
            Type::from(tpe)
        };

        env.fluents.remove(|id, _| group.member(id).is_some());
        let fluent = env.fluents.add_fluent(name, params, value_type, None).msg(env)?;
        Ok(LiftedGroup { group, fluent, helpers })
    }

    /// Returns the arguments and the value of the state variable representing the atom `predicate(args)`.
    fn encode(&self, predicate: FluentId, args: &[ExprId], env: &mut Environment) -> Res<(SeqExprId, ExprId)> {
        let (idx, member) = self
            .group
            .members
            .iter()
            .find_position(|m| m.fluent == predicate)
            .unwrap();
        let key = member.key_of(args).into();
        let value = match member.value_of(args) {
            Some(value) => value,
            None => env.intern(Expr::Object(self.helpers[idx].clone()), None)?,
        };
        Ok((key, value))
    }

    fn rewrite_effect(&self, eff: &mut Effect, assign: bool, env: &mut Environment) -> Res<()> {
        let eff = &mut eff.effect_expression;
        let sv = &mut eff.state_variable;
        let (key, value) = self.encode(sv.fluent, &sv.arguments, env)?;
        sv.fluent = self.fluent;
        sv.arguments = key;
        eff.operation = if assign {
            EffectOp::Assign(value)
        } else {
            EffectOp::Erase
        };
        Ok(())
    }
}

/// Rewrites the model so that all atoms of the lifted groups are expressed with their state function:
///  - an atom `p(args)` in an expression is replaced by `(= sv(key) value)`,
///  - action effects are rewritten as specified by the updates,
///  - each initial atom `p(args)` is replaced by `sv(key) := value`.
pub fn rewrite(model: &mut Model, groups: &[(LiftedGroup, Vec<(Sym, ActionUpdates)>)]) -> Res<()> {
    let by_predicate: HashMap<FluentId, &LiftedGroup> = groups
        .iter()
        .flat_map(|(lifted, _)| lifted.group.members.iter().map(move |m| (m.fluent, lifted)))
        .collect();

    for (root, _) in expression_roots(model) {
        rewrite_expr(root, &by_predicate, &mut model.env)?;
    }

    // effects are rewritten in place, the removed ones are only deleted at the end to keep indices valid
    let mut removed: HashMap<&Sym, Vec<usize>> = HashMap::new();
    for (lifted, updates) in groups {
        for (action_name, updates) in updates {
            let action = model.actions.get_action_mut(action_name).unwrap();
            for update in updates {
                if let Some(i) = update.assign {
                    lifted.rewrite_effect(&mut action.effects[i], true, &mut model.env)?;
                }
                if let Some(i) = update.erase {
                    lifted.rewrite_effect(&mut action.effects[i], false, &mut model.env)?;
                }
                removed.entry(action_name).or_default().extend(&update.removed);
            }
        }
    }
    for (action_name, indices) in removed {
        let action = model.actions.get_action_mut(action_name).unwrap();
        remove_indices(&mut action.effects, indices.into_iter().collect());
    }

    // initial facts: duplicated atoms are removed to avoid two assignments of the same state variable
    let mut initialized = HashSet::new();
    let mut duplicates = HashSet::new();
    for (i, eff) in model.init.iter_mut().enumerate() {
        let sv = &eff.effect_expression.state_variable;
        let Some(lifted) = by_predicate.get(&sv.fluent) else {
            continue;
        };
        let member = lifted.group.member(sv.fluent).unwrap();
        let ground_key = Term::all_of(&member.key_of(&sv.arguments), &model.env);
        if initialized.insert((lifted.fluent, ground_key)) {
            lifted.rewrite_effect(eff, true, &mut model.env)?;
        } else {
            duplicates.insert(i);
        }
    }
    remove_indices(&mut model.init, duplicates);
    Ok(())
}

fn rewrite_expr(expr: ExprId, groups: &HashMap<FluentId, &LiftedGroup>, env: &mut Environment) -> Res<()> {
    if let Expr::StateVariable(predicate, args) = env.node(expr).expr()
        && let Some(lifted) = groups.get(predicate)
    {
        let (predicate, args) = (*predicate, args.clone());
        let span = env.node(expr).span().cloned();
        let (key, value) = lifted.encode(predicate, &args, env)?;
        let sv = env.intern(Expr::StateVariable(lifted.fluent, key), span.clone())?;
        env.replace(expr, Expr::App(Fun::Eq, smallvec![sv, value]), span)?;
    }
    for child in children(expr, env) {
        rewrite_expr(child, groups, env)?;
    }
    Ok(())
}

fn remove_indices<T>(elems: &mut Vec<T>, indices: HashSet<usize>) {
    let mut i = 0;
    elems.retain(|_| {
        i += 1;
        !indices.contains(&(i - 1))
    });
}
