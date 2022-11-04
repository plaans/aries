use anyhow::{ensure, Context, Result};
use unified_planning::{effect_expression::EffectKind, Condition, ExpressionKind};

use crate::models::{condition::ValCondition, effect::ValEffect, env::Env, expression::ValExpression, value::Value};

impl ValEffect for unified_planning::Effect {
    fn condition(&self) -> Result<Option<Box<dyn ValCondition>>> {
        if let Some(cond) = &self.effect.as_ref().context("Effect without expression")?.condition {
            Ok(Some(Box::new(Condition {
                cond: Some(cond.clone()),
                span: None,
            })))
        } else {
            Ok(None)
        }
    }

    fn fluent(&self) -> Result<(String, Vec<Box<dyn ValExpression>>)> {
        let fluent = self
            .effect
            .as_ref()
            .context("Effect without expression")?
            .fluent
            .as_ref()
            .context("Effect without fluent")?;
        ensure!(matches!(fluent.kind(), ExpressionKind::StateVariable));
        let args = fluent
            .list
            .iter()
            .skip(1)
            .map(|a| Box::new(a.clone()) as Box<dyn ValExpression>)
            .collect::<Vec<_>>();
        let fluent = fluent.list.first().context("Fluent without symbol")?.symbol()?;
        Ok((fluent, args))
    }

    fn value(&self, env: &Env) -> Result<Value> {
        let value = self
            .effect
            .as_ref()
            .context("Effect without expression")?
            .value
            .as_ref()
            .context("Effect without value")?
            .eval(env)?;
        Ok(
            match self.effect.as_ref().context("Effect without expression")?.kind() {
                EffectKind::Assign => value,
                EffectKind::Increase => {
                    let (fluent, args) = self.fluent()?;
                    (&env.get_fluent(&fluent, &args)? + &value)?
                }
                EffectKind::Decrease => {
                    let (fluent, args) = self.fluent()?;
                    (&env.get_fluent(&fluent, &args)? - &value)?
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use unified_planning::{atom::Content, Atom, Effect, EffectExpression, Expression};

    use crate::interfaces::unified_planning::constants::UP_INTEGER;

    use super::*;

    #[test]
    fn value() -> Result<()> {
        let mut env = Env::default();
        env.bound_fluent(vec![Value::Symbol("f1".into())], Value::Number(10.into()));

        let assign = Effect {
            effect: Some(EffectExpression {
                kind: EffectKind::Assign.into(),
                fluent: Some(Expression {
                    list: vec![Expression {
                        atom: Some(Atom {
                            content: Some(Content::Symbol("f1".into())),
                        }),
                        kind: ExpressionKind::FluentSymbol.into(),
                        ..Default::default()
                    }],
                    kind: ExpressionKind::StateVariable.into(),
                    ..Default::default()
                }),
                value: Some(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Int(2)),
                    }),
                    r#type: UP_INTEGER.into(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            occurrence_time: None,
        };
        let mut increase = assign.clone();
        increase.effect.as_mut().unwrap().kind = EffectKind::Increase.into();
        let mut decrease = assign.clone();
        decrease.effect.as_mut().unwrap().kind = EffectKind::Decrease.into();

        assert_eq!(assign.value(&env)?, Value::Number(2.into()));
        assert_eq!(increase.value(&env)?, Value::Number(12.into()));
        assert_eq!(decrease.value(&env)?, Value::Number(8.into()));

        Ok(())
    }
}
