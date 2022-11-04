use anyhow::Result;

use super::{condition::ValCondition, env::Env, expression::ValExpression, value::Value};

/// The minimal behaviour of an effect.
pub trait ValEffect {
    /// Returns the optional condition associated to the effect.
    fn condition(&self) -> Result<Option<Box<dyn ValCondition>>>;
    /// Returns the fluent name and its non-evaluated arguments, which is changed by the effect.
    fn fluent(&self) -> Result<(String, Vec<Box<dyn ValExpression>>)>;
    /// Returns the new value of the value if the effect is applied.
    fn value(&self, env: &Env) -> Result<Value>;

    /// Returns whether or not the effect is applicable in the current environment.
    fn is_applicable(&self, env: &Env) -> Result<bool> {
        Ok(if let Some(condition) = self.condition()? {
            condition.is_valid(env)?
        } else {
            true
        })
    }
    /// Evaluates the signature of the fluent in the current environment.
    fn fluent_signature(&self, env: &Env) -> Result<Vec<Value>> {
        let (fluent, args) = self.fluent()?;
        args.iter()
            .fold::<Result<_>, _>(Ok(Vec::<Value>::from([Value::Symbol(fluent)])), |acc, arg| {
                let mut new_acc = acc?.to_vec();
                new_acc.push(arg.eval(env)?);
                Ok(new_acc)
            })
    }
}

#[cfg(test)]
mod tests {
    use unified_planning::{atom::Content, Atom, Effect, EffectExpression, Expression, ExpressionKind};

    use crate::interfaces::unified_planning::constants::UP_BOOL;

    use super::*;

    #[test]
    fn is_applicable() -> Result<()> {
        let t = Effect {
            effect: Some(EffectExpression {
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
            occurrence_time: None,
        };
        let f = Effect {
            effect: Some(EffectExpression {
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
            occurrence_time: None,
        };
        assert!(t.is_applicable(&Env::default())?);
        assert!(!f.is_applicable(&Env::default())?);
        Ok(())
    }

    #[test]
    fn fluent_signature() -> Result<()> {
        let mut env = Env::default();
        env.bound("t".into(), "o1".into(), Value::Symbol("o1".into()));
        env.bound("t".into(), "o2".into(), Value::Number(2.into()));

        let effect = Effect {
            effect: Some(EffectExpression {
                fluent: Some(Expression {
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
                                content: Some(Content::Symbol("o1".into())),
                            }),
                            kind: ExpressionKind::Parameter.into(),
                            ..Default::default()
                        },
                        Expression {
                            atom: Some(Atom {
                                content: Some(Content::Symbol("o2".into())),
                            }),
                            kind: ExpressionKind::Parameter.into(),
                            ..Default::default()
                        },
                    ],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            occurrence_time: None,
        };

        assert_eq!(
            effect.fluent_signature(&env)?,
            vec![
                Value::Symbol("f1".into()),
                Value::Symbol("o1".into()),
                Value::Number(2.into())
            ]
        );

        Ok(())
    }
}
