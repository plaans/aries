use crate::models::{action::ValAction, env::Env, expression::ValExpression, problem::ValProblem, value::Value};
use anyhow::{bail, ensure, Context, Result};

impl ValProblem for unified_planning::Problem {
    fn initial_env(&self, verbose: bool) -> Result<Env> {
        let mut env = Env::default();
        env.verbose = verbose;
        for o in self.objects.iter() {
            env.bound(o.r#type.clone(), o.name.clone(), Value::Symbol(o.name.clone()));
        }
        for f in self.fluents.iter() {
            if let Some(default) = &f.default_value {
                env.add_default_fluent(f.name.clone(), default.eval(&env)?);
            }
        }
        for assignment in self.initial_state.iter() {
            let fluent = assignment.fluent.as_ref().context("No fluent in the assignment")?;
            ensure!(matches!(fluent.kind(), unified_planning::ExpressionKind::StateVariable));
            let fluent = fluent.list.iter().skip(1).fold::<Result<_>, _>(
                Ok(Vec::<Value>::from([Value::Symbol(
                    fluent.list.first().context("No fluent symbol")?.symbol()?,
                )])),
                |acc, arg| {
                    let mut new_acc = acc?.to_vec();
                    new_acc.push(arg.eval(&env)?);
                    Ok(new_acc)
                },
            )?;
            let value = assignment
                .value
                .as_ref()
                .context("No value in the assignment")?
                .eval(&env)?;
            env.bound_fluent(fluent, value);
        }
        Ok(env)
    }

    fn get_action(&self, name: String) -> Result<Box<dyn ValAction>> {
        for action in self.actions.iter() {
            if action.name == name {
                return Ok(Box::new(action.clone()) as Box<dyn ValAction>);
            }
        }
        bail!(format!("No action named {:?}", name))
    }
}

#[cfg(test)]
mod tests {
    use unified_planning::{
        atom::Content, Action, Assignment, Atom, Expression, ExpressionKind, Fluent, ObjectDeclaration, Problem,
    };

    use crate::{interfaces::unified_planning::constants::UP_BOOL, models::state::State};

    use super::*;

    #[test]
    fn into_env() -> Result<()> {
        let problem = Problem {
            fluents: vec![
                Fluent {
                    name: "f1".into(),
                    value_type: UP_BOOL.into(),
                    default_value: Some(Expression {
                        atom: Some(Atom {
                            content: Some(Content::Boolean(true)),
                        }),
                        r#type: UP_BOOL.into(),
                        kind: ExpressionKind::Constant.into(),
                        ..Default::default()
                    }),
                    parameters: vec![],
                },
                Fluent {
                    name: "f2".into(),
                    value_type: UP_BOOL.into(),
                    default_value: Some(Expression {
                        atom: Some(Atom {
                            content: Some(Content::Boolean(false)),
                        }),
                        r#type: UP_BOOL.into(),
                        kind: ExpressionKind::Constant.into(),
                        ..Default::default()
                    }),
                    parameters: vec![],
                },
            ],
            objects: vec![
                ObjectDeclaration {
                    name: "R1".into(),
                    r#type: "r".into(),
                },
                ObjectDeclaration {
                    name: "R2".into(),
                    r#type: "r".into(),
                },
            ],
            initial_state: vec![Assignment {
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f2".into())),
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
            }],
            ..Default::default()
        };
        let mut expected_state = State::default();
        expected_state.bound(vec![Value::Symbol("f2".into())], Value::Bool(true));
        let mut expected_env = Env::default();
        expected_env.add_default_fluent("f1".into(), Value::Bool(true));
        expected_env.add_default_fluent("f2".into(), Value::Bool(false));
        expected_env.bound("r".into(), "R1".into(), Value::Symbol("R1".into()));
        expected_env.bound("r".into(), "R2".into(), Value::Symbol("R2".into()));
        expected_env.update_state(expected_state);
        assert_eq!(expected_env, problem.initial_env(false)?);
        Ok(())
    }

    #[test]
    fn get_action() -> Result<()> {
        let action = Action {
            name: "a1".into(),
            ..Default::default()
        };
        let problem = Problem {
            actions: vec![action.clone()],
            ..Default::default()
        };
        assert_eq!(problem.get_action("a1".into())?.name()?, action.name);
        assert!(problem.get_action("a2".into()).is_err());
        Ok(())
    }
}
