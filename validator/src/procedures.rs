use crate::models::{env::Env, expression::ValExpression, value::Value};

use anyhow::{bail, ensure, Result};

pub fn and(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    args.iter().fold(Ok(Value::Bool(true)), |r, a| match r {
        Ok(b) => &b & &a.eval(env)?,
        Err(e) => Err(e),
    })
}

pub fn or(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    args.iter().fold(Ok(Value::Bool(false)), |r, a| match r {
        Ok(b) => &b | &a.eval(env)?,
        Err(e) => Err(e),
    })
}

pub fn not(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 1);
    !&args.first().unwrap().eval(env)?
}

// A implies B  <==>  (not A) or B
pub fn implies(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    &(!&a)? | &b
}

pub fn equals(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    Ok(Value::Bool(a == b))
}

pub fn le(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    Ok(match a {
        Value::Number(v1) => match b {
            Value::Number(v2) => Value::Bool(v1 <= v2),
            _ => bail!("<= operation with a non-number value"),
        },
        _ => bail!("<= operation with a non-number value"),
    })
}

pub fn plus(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    &a + &b
}

pub fn minus(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    &a - &b
}

pub fn times(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    &a * &b
}

pub fn div(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let a = args.get(0).unwrap().eval(env)?;
    let b = args.get(1).unwrap().eval(env)?;
    &a / &b
}

pub fn exists(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let var = args.get(0).unwrap();
    let expr = args.get(1).unwrap();
    for o in env.get_objects(&var.tpe()?)? {
        let mut new_env = env.clone();
        new_env.bound(var.tpe()?.clone(), var.symbol()?.clone(), o.clone());
        if expr.eval(&new_env)? == Value::Bool(true) {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}

pub fn forall(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
    ensure!(args.len() == 2);
    let var = args.get(0).unwrap();
    let expr = args.get(1).unwrap();
    for o in env.get_objects(&var.tpe()?)? {
        let mut new_env = env.clone();
        new_env.bound(var.tpe()?.clone(), var.symbol()?.clone(), o.clone());
        if expr.eval(&new_env)? == Value::Bool(false) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

#[cfg(test)]
mod tests {
    use unified_planning::{atom::Content, Atom, Expression, ExpressionKind};

    use crate::{
        interfaces::unified_planning::constants::{UP_BOOL, UP_INTEGER},
        models::state::State,
    };

    use super::*;

    #[test]
    fn test_and() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let f = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(false)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let b = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(and(&env, &[Box::new(b.clone())]).is_err());
        assert_eq!(and(&env, &[Box::new(t.clone())])?, Value::Bool(true));
        assert_eq!(
            and(&env, &[Box::new(t.clone()), Box::new(t.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            and(&env, &[Box::new(t.clone()), Box::new(f.clone())])?,
            Value::Bool(false)
        );
        assert_eq!(
            and(&env, &[Box::new(f.clone()), Box::new(t.clone())])?,
            Value::Bool(false)
        );
        assert_eq!(
            and(&env, &[Box::new(f.clone()), Box::new(f.clone())])?,
            Value::Bool(false)
        );
        assert_eq!(
            and(&env, &[Box::new(t.clone()), Box::new(t.clone()), Box::new(t.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            and(&env, &[Box::new(f.clone()), Box::new(t.clone()), Box::new(t.clone())])?,
            Value::Bool(false)
        );
        Ok(())
    }

    #[test]
    fn test_or() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let f = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(false)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let b = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(or(&env, &[Box::new(b.clone())]).is_err());
        assert_eq!(or(&env, &[Box::new(t.clone())])?, Value::Bool(true));
        assert_eq!(
            or(&env, &[Box::new(t.clone()), Box::new(t.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            or(&env, &[Box::new(t.clone()), Box::new(f.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            or(&env, &[Box::new(f.clone()), Box::new(t.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            or(&env, &[Box::new(f.clone()), Box::new(f.clone())])?,
            Value::Bool(false)
        );
        assert_eq!(
            or(&env, &[Box::new(f.clone()), Box::new(t.clone()), Box::new(t.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            or(&env, &[Box::new(f.clone()), Box::new(f.clone()), Box::new(f.clone())])?,
            Value::Bool(false)
        );
        Ok(())
    }

    #[test]
    fn test_not() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let f = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(false)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(not(&env, &[]).is_err());
        assert!(not(&env, &[Box::new(t.clone()), Box::new(t.clone())]).is_err());
        assert_eq!(not(&env, &[Box::new(t.clone())])?, Value::Bool(false));
        assert_eq!(not(&env, &[Box::new(f.clone())])?, Value::Bool(true));
        Ok(())
    }

    #[test]
    fn test_implies() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let f = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(false)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(implies(&env, &[]).is_err());
        assert!(implies(&env, &[Box::new(t.clone())]).is_err());
        assert!(implies(&env, &[Box::new(t.clone()), Box::new(t.clone()), Box::new(t.clone())]).is_err());
        assert_eq!(
            implies(&env, &[Box::new(t.clone()), Box::new(t.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            implies(&env, &[Box::new(f.clone()), Box::new(t.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            implies(&env, &[Box::new(t.clone()), Box::new(f.clone())])?,
            Value::Bool(false)
        );
        assert_eq!(
            implies(&env, &[Box::new(f.clone()), Box::new(f.clone())])?,
            Value::Bool(true)
        );
        Ok(())
    }

    #[test]
    fn test_equals() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let f = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(false)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i1 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(1)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i2 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(equals(&env, &[]).is_err());
        assert!(equals(&env, &[Box::new(t.clone())]).is_err());
        assert!(equals(&env, &[Box::new(t.clone()), Box::new(t.clone()), Box::new(t.clone())]).is_err());
        assert_eq!(
            equals(&env, &[Box::new(t.clone()), Box::new(t.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            equals(&env, &[Box::new(f.clone()), Box::new(t.clone())])?,
            Value::Bool(false)
        );
        assert_eq!(
            equals(&env, &[Box::new(i1.clone()), Box::new(i1.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            equals(&env, &[Box::new(i1.clone()), Box::new(i2.clone())])?,
            Value::Bool(false)
        );
        assert_eq!(
            equals(&env, &[Box::new(t.clone()), Box::new(i2.clone())])?,
            Value::Bool(false)
        );
        Ok(())
    }

    #[test]
    fn test_le() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i1 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(1)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i2 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(le(&env, &[]).is_err());
        assert!(le(&env, &[Box::new(i1.clone())]).is_err());
        assert!(le(
            &env,
            &[Box::new(i1.clone()), Box::new(i1.clone()), Box::new(i1.clone())]
        )
        .is_err());
        assert!(le(&env, &[Box::new(i1.clone()), Box::new(t.clone())]).is_err());
        assert!(le(&env, &[Box::new(t.clone()), Box::new(i1.clone())]).is_err());
        assert_eq!(
            le(&env, &[Box::new(i1.clone()), Box::new(i2.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            le(&env, &[Box::new(i2.clone()), Box::new(i1.clone())])?,
            Value::Bool(false)
        );
        assert_eq!(
            le(&env, &[Box::new(i1.clone()), Box::new(i1.clone())])?,
            Value::Bool(true)
        );
        Ok(())
    }

    #[test]
    fn test_plus() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i1 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(1)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i2 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(plus(&env, &[]).is_err());
        assert!(plus(&env, &[Box::new(i1.clone())]).is_err());
        assert!(plus(
            &env,
            &[Box::new(i1.clone()), Box::new(i1.clone()), Box::new(i1.clone())]
        )
        .is_err());
        assert!(plus(&env, &[Box::new(i1.clone()), Box::new(t.clone())]).is_err());
        assert!(plus(&env, &[Box::new(t.clone()), Box::new(i1.clone())]).is_err());
        assert_eq!(
            plus(&env, &[Box::new(i1.clone()), Box::new(i2.clone())])?,
            Value::Number(3.into())
        );
        Ok(())
    }

    #[test]
    fn test_minus() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i1 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(1)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i2 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(minus(&env, &[]).is_err());
        assert!(minus(&env, &[Box::new(i1.clone())]).is_err());
        assert!(minus(
            &env,
            &[Box::new(i1.clone()), Box::new(i1.clone()), Box::new(i1.clone())]
        )
        .is_err());
        assert!(minus(&env, &[Box::new(i1.clone()), Box::new(t.clone())]).is_err());
        assert!(minus(&env, &[Box::new(t.clone()), Box::new(i1.clone())]).is_err());
        assert_eq!(
            minus(&env, &[Box::new(i1.clone()), Box::new(i2.clone())])?,
            Value::Number((-1).into())
        );
        Ok(())
    }

    #[test]
    fn test_times() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i1 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(3)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i2 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(times(&env, &[]).is_err());
        assert!(times(&env, &[Box::new(i1.clone())]).is_err());
        assert!(times(
            &env,
            &[Box::new(i1.clone()), Box::new(i1.clone()), Box::new(i1.clone())]
        )
        .is_err());
        assert!(times(&env, &[Box::new(i1.clone()), Box::new(t.clone())]).is_err());
        assert!(times(&env, &[Box::new(t.clone()), Box::new(i1.clone())]).is_err());
        assert_eq!(
            times(&env, &[Box::new(i1.clone()), Box::new(i2.clone())])?,
            Value::Number(6.into())
        );
        Ok(())
    }

    #[test]
    fn test_div() -> Result<()> {
        let env = Env::default();
        let t = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i1 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(6)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i2 = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert!(div(&env, &[]).is_err());
        assert!(div(&env, &[Box::new(i1.clone())]).is_err());
        assert!(div(
            &env,
            &[Box::new(i1.clone()), Box::new(i1.clone()), Box::new(i1.clone())]
        )
        .is_err());
        assert!(div(&env, &[Box::new(i1.clone()), Box::new(t.clone())]).is_err());
        assert!(div(&env, &[Box::new(t.clone()), Box::new(i1.clone())]).is_err());
        assert_eq!(
            div(&env, &[Box::new(i1.clone()), Box::new(i2.clone())])?,
            Value::Number(3.into())
        );
        Ok(())
    }

    #[test]
    fn test_exists() -> Result<()> {
        let mut state = State::default();
        state.bound(
            vec![Value::Symbol("f1".into()), Value::Symbol("o1".into())],
            Value::Bool(true),
        );
        state.bound(
            vec![Value::Symbol("f1".into()), Value::Symbol("o2".into())],
            Value::Bool(false),
        );
        state.bound(
            vec![Value::Symbol("f2".into()), Value::Symbol("o1".into())],
            Value::Bool(false),
        );
        state.bound(
            vec![Value::Symbol("f2".into()), Value::Symbol("o2".into())],
            Value::Bool(false),
        );

        let mut env = Env::default();
        env.update_state(state);
        env.bound("t".into(), "o1".into(), Value::Symbol("o1".into()));
        env.bound("t".into(), "o2".into(), Value::Symbol("o2".into()));

        let var = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("o".into())),
            }),
            r#type: "t".into(),
            kind: ExpressionKind::Variable.into(),
            ..Default::default()
        };
        let e1 = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("f1".into())),
                    }),
                    kind: ExpressionKind::FluentSymbol.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("o".into())),
                    }),
                    r#type: "t".into(),
                    kind: ExpressionKind::Parameter.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::StateVariable.into(),
            ..Default::default()
        };
        let e2 = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("f2".into())),
                    }),
                    kind: ExpressionKind::FluentSymbol.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("o".into())),
                    }),
                    r#type: "t".into(),
                    kind: ExpressionKind::Parameter.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::StateVariable.into(),
            ..Default::default()
        };
        assert!(exists(&env, &[]).is_err());
        assert!(exists(&env, &[Box::new(var.clone())]).is_err());
        assert!(exists(
            &env,
            &[Box::new(var.clone()), Box::new(var.clone()), Box::new(var.clone())]
        )
        .is_err());
        assert_eq!(
            exists(&env, &[Box::new(var.clone()), Box::new(e1.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            exists(&env, &[Box::new(var.clone()), Box::new(e2.clone())])?,
            Value::Bool(false)
        );
        Ok(())
    }

    #[test]
    fn test_forall() -> Result<()> {
        let mut state = State::default();
        state.bound(
            vec![Value::Symbol("f1".into()), Value::Symbol("o1".into())],
            Value::Bool(true),
        );
        state.bound(
            vec![Value::Symbol("f1".into()), Value::Symbol("o2".into())],
            Value::Bool(true),
        );
        state.bound(
            vec![Value::Symbol("f2".into()), Value::Symbol("o1".into())],
            Value::Bool(true),
        );
        state.bound(
            vec![Value::Symbol("f2".into()), Value::Symbol("o2".into())],
            Value::Bool(false),
        );

        let mut env = Env::default();
        env.update_state(state);
        env.bound("t".into(), "o1".into(), Value::Symbol("o1".into()));
        env.bound("t".into(), "o2".into(), Value::Symbol("o2".into()));

        let var = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("o".into())),
            }),
            r#type: "t".into(),
            kind: ExpressionKind::Variable.into(),
            ..Default::default()
        };
        let e1 = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("f1".into())),
                    }),
                    kind: ExpressionKind::FluentSymbol.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("o".into())),
                    }),
                    r#type: "t".into(),
                    kind: ExpressionKind::Parameter.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::StateVariable.into(),
            ..Default::default()
        };
        let e2 = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("f2".into())),
                    }),
                    kind: ExpressionKind::FluentSymbol.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("o".into())),
                    }),
                    r#type: "t".into(),
                    kind: ExpressionKind::Parameter.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::StateVariable.into(),
            ..Default::default()
        };
        assert!(forall(&env, &[]).is_err());
        assert!(forall(&env, &[Box::new(var.clone())]).is_err());
        assert!(forall(
            &env,
            &[Box::new(var.clone()), Box::new(var.clone()), Box::new(var.clone())]
        )
        .is_err());
        assert_eq!(
            forall(&env, &[Box::new(var.clone()), Box::new(e1.clone())])?,
            Value::Bool(true)
        );
        assert_eq!(
            forall(&env, &[Box::new(var.clone()), Box::new(e2.clone())])?,
            Value::Bool(false)
        );
        Ok(())
    }
}
