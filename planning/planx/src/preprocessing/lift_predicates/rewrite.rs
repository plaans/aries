//! Rewriting of a model, replacing the predicates of a group by a single state function.

use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use smallvec::smallvec;

use super::groups::{ActionUpdates, Group, KeyArg, supertype};
use crate::errors::{EnvError, Spanned};
use crate::preprocessing::analysis::*;
use crate::*;

/// A group whose predicates have been replaced by a state function in the environment.
pub struct LiftedGroup {
    pub group: Group,
    /// The state function replacing the group's predicates.
    pub fluent: FluentId,
    /// For members without value parameter: the value representing them (same order as `group.members`).
    helpers: Vec<Option<Object>>,
}

impl LiftedGroup {
    /// Removes the group's predicates from the environment and declares the state function (and helper objects) replacing them.
    pub fn declare(group: Group, name: &str, env: &mut Environment) -> Res<Self> {
        let param = |fluent: FluentId, i: usize| env.fluents.get(fluent).parameters[i].clone();

        // parameters of the state function: for each key position, the parameter with the most general type
        let mut params = Vec::new();
        for k in 0..group.members[0].key.len() {
            let candidates = (group.members.iter())
                .map(|m| match m.key[k] {
                    KeyArg::Arg(i) => param(m.fluent, i),
                    KeyArg::Fun(f, _) => param(f.fluent, f.output()),
                })
                .collect_vec();
            let tpe = supertype(candidates.iter().map(|p| p.tpe().clone())).unwrap();
            params.push(candidates.into_iter().find(|p| p.tpe() == &tpe).unwrap());
        }

        let value_types = (group.members.iter())
            .filter_map(|m| m.value.map(|v| param(m.fluent, v).tpe().clone()))
            .collect_vec();
        let mut helpers = Vec::new();
        let value_type = if group.members.iter().all(|m| m.value.is_some()) {
            supertype(value_types).unwrap_or_else(|| Type::from(env.types.top_user_type()))
        } else {
            let tpe_name = format!("_help-tpe-{name}");
            env.types.add_top_type_child(tpe_name.as_str())?;
            let tpe = env.types.get_user_type(tpe_name.as_str()).msg(env)?;
            for m in &group.members {
                if m.value.is_none() {
                    let obj_name = format!("_help-obj-{}", env.fluents.get(m.fluent).name());
                    env.objects.add_object(obj_name.as_str(), tpe.clone()).msg(env)?;
                    helpers.push(Some(env.objects.get(obj_name.as_str()).msg(env)?));
                } else {
                    helpers.push(None);
                }
            }
            if value_types.is_empty() {
                Type::from(tpe)
            } else {
                Type::from(env.types.top_user_type())
            }
        };

        env.fluents.remove(|id, _| group.member(id).is_some());
        let fluent = env.fluents.add_fluent(name, params, value_type, None).msg(env)?;
        Ok(LiftedGroup { group, fluent, helpers })
    }

    /// Returns the arguments and the value of the state variable representing the atom `predicate(args)`.
    fn encode(
        &self,
        predicate: FluentId,
        args: &[ExprId],
        bindings: &Bindings,
        env: &mut Environment,
    ) -> Res<(SeqExprId, ExprId)> {
        let (idx, member) = (self.group.members.iter())
            .find_position(|m| m.fluent == predicate)
            .unwrap();
        let key = member
            .key_of(args, |f, &input| bindings.resolve(f, input, env))
            .expect("functional keys are computable (checked by the analysis)");
        let value = match (member.value_of(args), &self.helpers.get(idx)) {
            (Some(value), _) => value,
            (None, Some(Some(helper))) => env.intern(Expr::Object(helper.clone()), None)?,
            _ => unreachable!(),
        };
        Ok((key.into(), value))
    }

    fn rewrite_effect(&self, eff: &mut Effect, assign: bool, bindings: &Bindings, env: &mut Environment) -> Res<()> {
        let eff = &mut eff.effect_expression;
        let sv = &mut eff.state_variable;
        let (key, value) = self.encode(sv.fluent, &sv.arguments, bindings, env)?;
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

/// Evaluates static functions on expressions, either in the context of an action (whose conditions give their value
/// on parameters) or of the problem (on objects only).
struct Bindings<'f> {
    /// Atoms required by the action (empty outside of an action).
    atoms: Vec<(FluentId, SeqExprId)>,
    functions: &'f StaticFunctions,
}

impl<'f> Bindings<'f> {
    fn global(functions: &'f StaticFunctions) -> Self {
        Bindings {
            atoms: Vec::new(),
            functions,
        }
    }

    /// Bindings given by the conditions of the action, in the same order as considered by the analysis.
    fn of_action(action: &Action, env: &Environment, functions: &'f StaticFunctions) -> Self {
        let atoms = (action.conditions.iter())
            .flat_map(|c| conjuncts(c.cond, env))
            .filter_map(|e| match env.node(e).expr() {
                Expr::StateVariable(fluent, args) if Term::all_of(args, env).is_some() => Some((*fluent, args.clone())),
                _ => None,
            })
            .collect();
        Bindings { atoms, functions }
    }

    fn resolve(&self, f: &StaticFunction, input: ExprId, env: &mut Environment) -> Option<ExprId> {
        match Term::of(input, env)? {
            Term::Object(o) => {
                let object = env.objects.get(self.functions.eval(f, &o)?.clone()).ok()?;
                env.intern(Expr::Object(object), None).ok()
            }
            input => (self.atoms.iter())
                .filter(|(fluent, _)| *fluent == f.fluent)
                .find_map(|(_, args)| {
                    f.apply(&Term::all_of(args, env).unwrap(), &input)
                        .map(|_| args[f.output()])
                }),
        }
    }
}

/// Rewrites the model so that all atoms of the lifted groups are expressed with their state function:
///  - an atom `p(args)` in an expression is replaced by `(= sv(key) value)`,
///  - action effects are rewritten as specified by the updates,
///  - each initial atom `p(args)` is replaced by `sv(key) := value`.
pub fn rewrite(
    model: &mut Model,
    groups: &[(LiftedGroup, Vec<(Sym, ActionUpdates)>)],
    functions: &StaticFunctions,
) -> Res<()> {
    let by_predicate: HashMap<FluentId, &LiftedGroup> = groups
        .iter()
        .flat_map(|(lifted, _)| lifted.group.members.iter().map(move |m| (m.fluent, lifted)))
        .collect();

    // collected before any rewriting, as the static functions may themselves be lifted
    let global = Bindings::global(functions);
    let action_bindings: HashMap<Sym, Bindings> = (model.actions.iter())
        .map(|a| (a.name.clone(), Bindings::of_action(a, &model.env, functions)))
        .collect();

    for (root, _) in model_roots(model) {
        rewrite_expr(root, &by_predicate, &global, &mut model.env)?;
    }
    for action in model.actions.iter() {
        for (root, _) in action_roots(action) {
            rewrite_expr(root, &by_predicate, &action_bindings[&action.name], &mut model.env)?;
        }
    }

    // effects are rewritten in place, the removed ones are only deleted at the end to keep indices valid
    let mut removed: HashMap<&Sym, Vec<usize>> = HashMap::new();
    for (lifted, updates) in groups {
        for (action_name, updates) in updates {
            let action = model.actions.get_action_mut(action_name).unwrap();
            let bindings = &action_bindings[action_name];
            for update in updates {
                if let Some(i) = update.assign {
                    lifted.rewrite_effect(&mut action.effects[i], true, bindings, &mut model.env)?;
                }
                if let Some(i) = update.erase {
                    lifted.rewrite_effect(&mut action.effects[i], false, bindings, &mut model.env)?;
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
        let (key, _) = lifted.encode(sv.fluent, &sv.arguments.clone(), &global, &mut model.env)?;
        if initialized.insert((lifted.fluent, Term::all_of(&key, &model.env))) {
            lifted.rewrite_effect(eff, true, &global, &mut model.env)?;
        } else {
            duplicates.insert(i);
        }
    }
    remove_indices(&mut model.init, duplicates);
    Ok(())
}

fn rewrite_expr(
    expr: ExprId,
    groups: &HashMap<FluentId, &LiftedGroup>,
    bindings: &Bindings,
    env: &mut Environment,
) -> Res<()> {
    if let Expr::StateVariable(predicate, args) = env.node(expr).expr()
        && let Some(lifted) = groups.get(predicate)
    {
        let (predicate, args) = (*predicate, args.clone());
        let span = env.node(expr).span().cloned();
        let (key, value) = lifted.encode(predicate, &args, bindings, env)?;
        let sv = env.intern(Expr::StateVariable(lifted.fluent, key), span.clone())?;
        env.replace(expr, Expr::App(Fun::Eq, smallvec![sv, value]), span)?;
    }
    for child in children(expr, env) {
        rewrite_expr(child, groups, bindings, env)?;
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
