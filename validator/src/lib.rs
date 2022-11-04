mod interfaces;
mod macros;
mod models;
mod procedures;

use anyhow::{bail, ensure, Result};
use models::{
    action::ValAction, condition::ValCondition, effect::ValEffect, env::Env, plan::ValPlan, problem::ValProblem,
};

/// Checks that the given plan is valid for the given problem.
pub fn validate(problem: &impl ValProblem, plan: &impl ValPlan, verbose: bool) -> Result<()> {
    print_info!(verbose, "Creation of the initial state");
    let mut env = problem.initial_env(verbose)?;

    print_info!(verbose, "Simulation of the plan");
    // TODO: Reason on effects and timeline rather than sequences.
    for action in plan.iter()? {
        let pb_action = problem.get_action(action.name()?)?;
        let mut new_env = env.clone();
        extend_env_with_action(&mut new_env, action.as_ref(), pb_action.as_ref())?;
        check_conditions(&new_env, &pb_action.conditions()?)?;
        check_effects(&new_env, &pb_action.effects()?)?;
        apply_effects(&mut new_env, &pb_action.effects()?)?;
        env.update_state(new_env.get_state());
    }
    Ok(())
}

/// Checks that the conditions of the action are respected.
fn check_conditions(env: &Env, conditions: &Vec<Box<dyn ValCondition>>) -> Result<()> {
    for condition in conditions {
        if !condition.is_valid(env)? {
            bail!("A condition is not respected")
        }
    }
    Ok(())
}

/// Checks that the effects don't interfere.
fn check_effects(env: &Env, effects: &Vec<Box<dyn ValEffect>>) -> Result<()> {
    let mut signatures = Vec::new();
    for effect in effects {
        if !effect.is_applicable(env)? {
            continue;
        }
        let sign = effect.fluent_signature(env)?;
        if signatures.contains(&sign) {
            bail!("A state variable is changed by two different effects")
        }
        signatures.push(sign);
    }
    Ok(())
}

/// Applies the effects.
fn apply_effects(env: &mut Env, effects: &Vec<Box<dyn ValEffect>>) -> Result<()> {
    let old_env = &env.clone();
    for effect in effects {
        if effect.is_applicable(old_env)? {
            env.bound_fluent(effect.fluent_signature(env)?, effect.value(env)?);
        }
    }
    Ok(())
}

/// Extends the environment with the parameters of the actions.
fn extend_env_with_action(env: &mut Env, action: &dyn ValAction, pb_action: &dyn ValAction) -> Result<()> {
    let parameters = action.parameters()?;
    let pb_parameters = pb_action.parameters()?;
    ensure!(pb_parameters.len() == parameters.len());
    for i in 0..parameters.len() {
        let param = parameters.get(i).unwrap();
        let pb_param = pb_parameters.get(i).unwrap();
        env.bound(pb_param.tpe()?, pb_param.symbol()?, param.eval(env)?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use unified_planning::{
        atom::Content, effect_expression::EffectKind, Action, ActionInstance, Atom, Condition, Effect,
        EffectExpression, Expression, ExpressionKind, Parameter,
    };

    use crate::{
        interfaces::unified_planning::constants::{UP_BOOL, UP_INTEGER},
        models::value::Value,
    };

    use super::*;

    #[test]
    fn test_check_conditions() -> Result<()> {
        let env = Env::default();
        let t = Condition {
            cond: Some(Expression {
                atom: Some(Atom {
                    content: Some(Content::Boolean(true)),
                }),
                r#type: UP_BOOL.into(),
                kind: ExpressionKind::Constant.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let f = Condition {
            cond: Some(Expression {
                atom: Some(Atom {
                    content: Some(Content::Boolean(false)),
                }),
                r#type: UP_BOOL.into(),
                kind: ExpressionKind::Constant.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(check_conditions(
            &env,
            &vec![Box::new(t.clone()), Box::new(t.clone()), Box::new(t.clone())]
        )
        .is_ok());
        assert!(check_conditions(
            &env,
            &vec![Box::new(t.clone()), Box::new(t.clone()), Box::new(f.clone())]
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn test_check_effects() -> Result<()> {
        let env = Env::default();
        let effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let cond_t_effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                condition: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(true)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let cond_f_effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                condition: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(false)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(check_effects(&env, &vec![Box::new(effect.clone())]).is_ok());
        assert!(check_effects(&env, &vec![Box::new(effect.clone()), Box::new(effect.clone())]).is_err());
        assert!(check_effects(&env, &vec![Box::new(effect.clone()), Box::new(cond_f_effect.clone())]).is_ok());
        assert!(check_effects(&env, &vec![Box::new(effect.clone()), Box::new(cond_t_effect.clone())]).is_err());
        Ok(())
    }

    #[test]
    fn test_apply_effects() -> Result<()> {
        let mut env = Env::default();
        env.bound_fluent(vec![Value::Symbol("f".into())], Value::Number(0.into()));
        env.bound_fluent(vec![Value::Symbol("a".into())], Value::Bool(false));
        let effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                value: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Int(1)),
                    }),
                    r#type: UP_INTEGER.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
                kind: EffectKind::Increase.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let cond_t_effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                value: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Int(1)),
                    }),
                    r#type: UP_INTEGER.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
                kind: EffectKind::Increase.into(),
                condition: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(true)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
            }),
            ..Default::default()
        };
        let cond_f_effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                value: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Int(1)),
                    }),
                    r#type: UP_INTEGER.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
                kind: EffectKind::Increase.into(),
                condition: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(false)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
            }),
            ..Default::default()
        };
        let a_effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("a".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                value: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(true)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let cond_a_effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                value: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Int(1)),
                    }),
                    r#type: UP_INTEGER.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
                kind: EffectKind::Increase.into(),
                condition: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("a".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
            }),
            ..Default::default()
        };
        assert_eq!(env.get_fluent(&"f".into(), &[])?, Value::Number(0.into()));
        apply_effects(&mut env, &vec![Box::new(effect.clone())])?;
        assert_eq!(env.get_fluent(&"f".into(), &[])?, Value::Number(1.into()));
        apply_effects(&mut env, &vec![Box::new(cond_f_effect.clone())])?;
        assert_eq!(env.get_fluent(&"f".into(), &[])?, Value::Number(1.into()));
        apply_effects(&mut env, &vec![Box::new(cond_t_effect.clone())])?;
        assert_eq!(env.get_fluent(&"f".into(), &[])?, Value::Number(2.into()));
        apply_effects(
            &mut env,
            &vec![Box::new(a_effect.clone()), Box::new(cond_a_effect.clone())],
        )?;
        assert_eq!(env.get_fluent(&"f".into(), &[])?, Value::Number(2.into()));
        Ok(())
    }

    #[test]
    fn test_extend_env_with_action() -> Result<()> {
        let mut env = Env::default();
        let action = ActionInstance {
            action_name: "a1".into(),
            parameters: vec![
                Atom {
                    content: Some(Content::Int(2)),
                },
                Atom {
                    content: Some(Content::Boolean(true)),
                },
                Atom {
                    content: Some(Content::Symbol("R1".into())),
                },
            ],
            ..Default::default()
        };
        let pb_action = Action {
            name: "a1".into(),
            parameters: vec![
                Parameter {
                    name: "p1".into(),
                    r#type: UP_INTEGER.into(),
                },
                Parameter {
                    name: "p2".into(),
                    r#type: UP_BOOL.into(),
                },
                Parameter {
                    name: "p3".into(),
                    r#type: "robot".into(),
                },
            ],
            ..Default::default()
        };
        extend_env_with_action(&mut env, &action, &pb_action)?;
        assert_eq!(env.get_var(&"p1".into())?, Value::Number(2.into()));
        assert_eq!(env.get_var(&"p2".into())?, Value::Bool(true));
        assert_eq!(env.get_var(&"p3".into())?, Value::Symbol("R1".into()));
        Ok(())
    }
}
