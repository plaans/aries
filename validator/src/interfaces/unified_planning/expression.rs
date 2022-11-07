use std::fmt::Write;

use crate::{
    interfaces::unified_planning::constants::{UP_BOOL, UP_INTEGER, UP_REAL},
    models::{env::Env, expression::ValExpression, value::Value},
    print_expr,
};
use anyhow::{bail, ensure, Context, Result};
use malachite::Rational;
use unified_planning::{atom::Content, Expression, ExpressionKind};

fn content(e: &Expression) -> Result<Content> {
    e.atom
        .clone()
        .context("No atom in the expression")?
        .content
        .context("No content in the atom")
}

/// Format the given expression for the console. If `k` is true, then the kind is printed.
fn fmt(e: &Expression, k: bool) -> Result<String> {
    let mut r = String::new();
    if k {
        write!(r, "{:?} ", e.kind())?;
    }
    match e.kind() {
        ExpressionKind::Constant => write!(r, "{:?}", content(e)?),
        ExpressionKind::Parameter | ExpressionKind::Variable => write!(r, "{} - {}", e.symbol()?, e.tpe()?),
        ExpressionKind::FluentSymbol | ExpressionKind::FunctionSymbol => write!(r, "{}", e.symbol()?),
        ExpressionKind::StateVariable | ExpressionKind::FunctionApplication => {
            write!(r, "{}", fmt(e.list.first().context("Missing head")?, false)?)?;
            let args = e
                .list
                .iter()
                .skip(1)
                .map(|a| fmt(a, false))
                .collect::<Result<Vec<_>>>()?;
            if !args.is_empty() {
                write!(r, " ( ")?;
                for i in 0..args.len() {
                    write!(r, "{}", args.get(i).unwrap())?;
                    if i != args.len() - 1 {
                        write!(r, " , ")?;
                    }
                }
                write!(r, " )")
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }?;
    Ok(r)
}

impl ValExpression for &unified_planning::Expression {
    fn eval(&self, env: &Env) -> Result<Value> {
        let value = match self.kind() {
            ExpressionKind::Unknown => bail!("Expression kind not specified"),
            ExpressionKind::Constant => match content(self)? {
                Content::Symbol(s) => Value::Symbol(s),
                Content::Int(i) => {
                    ensure!(self.r#type == UP_INTEGER);
                    Value::Number(i.into())
                }
                Content::Real(r) => {
                    ensure!(self.r#type == UP_REAL);
                    Value::Number(Rational::from_signeds(r.numerator, r.denominator))
                }
                Content::Boolean(b) => {
                    ensure!(self.r#type == UP_BOOL);
                    Value::Bool(b)
                }
            },
            ExpressionKind::Parameter | ExpressionKind::Variable => env.get_var(&self.symbol()?)?,
            ExpressionKind::FluentSymbol => bail!("Cannot evaluate a fluent symbol"),
            ExpressionKind::StateVariable => {
                let fluent = self
                    .list
                    .first()
                    .context("No fluent symbol in state variable expression")?;
                ensure!(matches!(fluent.kind(), ExpressionKind::FluentSymbol));
                let fluent = fluent.symbol()?;
                let args = self
                    .list
                    .iter()
                    .skip(1)
                    .map(|a| Box::new(a.clone()) as Box<dyn ValExpression>)
                    .collect::<Vec<_>>();
                env.get_fluent(&fluent, &args)?
            }
            ExpressionKind::FunctionSymbol => bail!("Cannot evaluate a function symbol"),
            ExpressionKind::FunctionApplication => {
                let procedure = self
                    .list
                    .first()
                    .context("No function symbol in function application expression")?;
                ensure!(matches!(procedure.kind(), ExpressionKind::FunctionSymbol));
                let procedure = procedure.symbol()?;
                let args = self
                    .list
                    .iter()
                    .skip(1)
                    .cloned()
                    .map(|x| Box::new(x) as Box<dyn ValExpression>)
                    .collect::<Vec<_>>();
                let c = env.get_procedure(&procedure)?;
                c(env, &args)?
            }
            ExpressionKind::ContainerId => bail!("Cannot evaluate a container id"),
        };

        print_expr!(env.verbose, "{} --> \x1b[1m{:?}\x1b[0m", fmt(self, true)?, value);
        Ok(value)
    }

    fn symbol(&self) -> Result<String> {
        Ok(match content(self)? {
            Content::Symbol(s) => s,
            _ => bail!("No symbol in the expression"),
        })
    }

    fn tpe(&self) -> Result<String> {
        Ok(self.r#type.clone())
    }
}

impl ValExpression for unified_planning::Expression {
    fn eval(&self, env: &Env) -> Result<Value> {
        (&self).eval(env)
    }

    fn symbol(&self) -> Result<String> {
        (&self).symbol()
    }

    fn tpe(&self) -> Result<String> {
        (&self).tpe()
    }
}

#[cfg(test)]
mod tests {
    use unified_planning::{Atom, Real};

    use crate::models::state::State;

    use super::*;

    #[test]
    fn eval_unknown() -> Result<()> {
        let env = Env::default();
        let e = Expression {
            kind: ExpressionKind::Unknown.into(),
            ..Default::default()
        };
        assert!(e.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_constant() -> Result<()> {
        let env = Env::default();
        let s = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("s".into())),
            }),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let i_bad = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let r = Expression {
            atom: Some(Atom {
                content: Some(Content::Real(Real {
                    numerator: 1,
                    denominator: 2,
                })),
            }),
            r#type: UP_REAL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let r_bad = Expression {
            atom: Some(Atom {
                content: Some(Content::Real(Real {
                    numerator: 1,
                    denominator: 2,
                })),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let b = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        let b_bad = Expression {
            atom: Some(Atom {
                content: Some(Content::Boolean(true)),
            }),
            r#type: UP_INTEGER.into(),
            kind: ExpressionKind::Constant.into(),
            ..Default::default()
        };
        assert_eq!(s.eval(&env)?, Value::Symbol("s".into()));
        assert_eq!(i.eval(&env)?, Value::Number(2.into()));
        assert_eq!(r.eval(&env)?, Value::Number(Rational::from_signeds(1, 2)));
        assert_eq!(b.eval(&env)?, Value::Bool(true));
        assert!(i_bad.eval(&env).is_err());
        assert!(r_bad.eval(&env).is_err());
        assert!(b_bad.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_parameter() -> Result<()> {
        let mut env = Env::default();
        env.bound("t".into(), "p".into(), Value::Number(2.into()));
        let param = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("p".into())),
            }),
            kind: ExpressionKind::Parameter.into(),
            ..Default::default()
        };
        let unbound = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("u".into())),
            }),
            kind: ExpressionKind::Parameter.into(),
            ..Default::default()
        };
        let bad = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            kind: ExpressionKind::Parameter.into(),
            ..Default::default()
        };
        assert_eq!(param.eval(&env)?, Value::Number(2.into()));
        assert!(unbound.eval(&env).is_err());
        assert!(bad.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_variable() -> Result<()> {
        let mut env = Env::default();
        env.bound("t".into(), "v".into(), Value::Number(2.into()));
        let var = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("v".into())),
            }),
            kind: ExpressionKind::Variable.into(),
            ..Default::default()
        };
        let unbound = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("u".into())),
            }),
            kind: ExpressionKind::Variable.into(),
            ..Default::default()
        };
        let bad = Expression {
            atom: Some(Atom {
                content: Some(Content::Int(2)),
            }),
            kind: ExpressionKind::Variable.into(),
            ..Default::default()
        };
        assert_eq!(var.eval(&env)?, Value::Number(2.into()));
        assert!(unbound.eval(&env).is_err());
        assert!(bad.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_fluent_symbol() -> Result<()> {
        let env = Env::default();
        let e = Expression {
            kind: ExpressionKind::FluentSymbol.into(),
            ..Default::default()
        };
        assert!(e.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_state_variable() -> Result<()> {
        let mut state = State::default();
        state.bound(
            vec![Value::Symbol("loc".into()), Value::Symbol("R1".into())],
            Value::Symbol("L3".into()),
        );
        let mut env = Env::default();
        env.bound("r".into(), "R1".into(), Value::Symbol("R1".into()));
        env.update_state(state);
        let expr = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("loc".into())),
                    }),
                    kind: ExpressionKind::FluentSymbol.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("R1".into())),
                    }),
                    kind: ExpressionKind::Parameter.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::StateVariable.into(),
            ..Default::default()
        };
        let bad = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("loc".into())),
                    }),
                    kind: ExpressionKind::Parameter.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("R1".into())),
                    }),
                    kind: ExpressionKind::Parameter.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::StateVariable.into(),
            ..Default::default()
        };
        assert_eq!(expr.eval(&env)?, Value::Symbol("L3".into()));
        assert!(bad.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_function_symbol() -> Result<()> {
        let env = Env::default();
        let e = Expression {
            kind: ExpressionKind::FunctionSymbol.into(),
            ..Default::default()
        };
        assert!(e.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_function_application() -> Result<()> {
        fn proc(env: &Env, args: &[Box<dyn ValExpression>]) -> Result<Value> {
            let a1 = args.first().unwrap().eval(env)?;
            let a2 = args.get(1).unwrap().eval(env)?;
            &(!a1)? & &a2
        }

        let mut env = Env::default();
        env.add_procedure("not".into(), proc);
        let expr = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("not".into())),
                    }),
                    kind: ExpressionKind::FunctionSymbol.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(false)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(true)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::FunctionApplication.into(),
            ..Default::default()
        };
        let bad = Expression {
            list: vec![
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol("not".into())),
                    }),
                    kind: ExpressionKind::Parameter.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(false)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                },
                Expression {
                    atom: Some(Atom {
                        content: Some(Content::Boolean(true)),
                    }),
                    r#type: UP_BOOL.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                },
            ],
            kind: ExpressionKind::FunctionApplication.into(),
            ..Default::default()
        };
        assert_eq!(expr.eval(&env)?, Value::Bool(true));
        assert!(bad.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn eval_container_id() -> Result<()> {
        let env = Env::default();
        let e = Expression {
            kind: ExpressionKind::ContainerId.into(),
            ..Default::default()
        };
        assert!(e.eval(&env).is_err());
        Ok(())
    }

    #[test]
    fn symbol() -> Result<()> {
        let expr = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("not".into())),
            }),
            kind: ExpressionKind::Parameter.into(),
            ..Default::default()
        };
        assert_eq!(expr.symbol()?, "not".to_string());
        Ok(())
    }

    #[test]
    fn tpe() -> Result<()> {
        let expr = Expression {
            atom: Some(Atom {
                content: Some(Content::Symbol("not".into())),
            }),
            r#type: UP_BOOL.into(),
            kind: ExpressionKind::Parameter.into(),
            ..Default::default()
        };
        assert_eq!(expr.tpe()?, UP_BOOL.to_string());
        Ok(())
    }
}
