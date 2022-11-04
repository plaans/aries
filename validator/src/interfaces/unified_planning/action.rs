use anyhow::{bail, Context, Result};
use unified_planning::{atom::Content, Atom, Expression, ExpressionKind};

use crate::models::{action::ValAction, condition::ValCondition, effect::ValEffect, expression::ValExpression};

use super::constants::{UP_BOOL, UP_INTEGER, UP_REAL};

impl ValAction for unified_planning::Action {
    fn conditions(&self) -> Result<Vec<Box<dyn ValCondition>>> {
        Ok(self
            .conditions
            .iter()
            .map(|c| Box::new(c.clone()) as Box<dyn ValCondition>)
            .collect::<Vec<_>>())
    }

    fn effects(&self) -> Result<Vec<Box<dyn ValEffect>>> {
        Ok(self
            .effects
            .iter()
            .map(|e| Box::new(e.clone()) as Box<dyn ValEffect>)
            .collect::<Vec<_>>())
    }

    fn name(&self) -> Result<String> {
        Ok(self.name.clone())
    }

    fn parameters(&self) -> Result<Vec<Box<dyn ValExpression>>> {
        Ok(self
            .parameters
            .iter()
            .map(|p| {
                Box::new(Expression {
                    atom: Some(Atom {
                        content: Some(Content::Symbol(p.name.clone())),
                    }),
                    r#type: p.r#type.clone(),
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }) as Box<dyn ValExpression>
            })
            .collect::<Vec<_>>())
    }
}

impl ValAction for unified_planning::ActionInstance {
    fn conditions(&self) -> Result<Vec<Box<dyn ValCondition>>> {
        bail!("Am ActionInstance has no conditions")
    }

    fn effects(&self) -> Result<Vec<Box<dyn ValEffect>>> {
        bail!("Am ActionInstance has no effects")
    }

    fn name(&self) -> Result<String> {
        Ok(self.action_name.clone())
    }

    fn parameters(&self) -> Result<Vec<Box<dyn ValExpression>>> {
        self.parameters
            .iter()
            .map(|a| {
                Ok(Box::new(Expression {
                    atom: Some(a.clone()),
                    r#type: match a.content.as_ref().context("Atom without content")? {
                        Content::Symbol(_) => "".into(),
                        Content::Int(_) => UP_INTEGER.into(),
                        Content::Real(_) => UP_REAL.into(),
                        Content::Boolean(_) => UP_BOOL.into(),
                    },
                    kind: ExpressionKind::Constant.into(),
                    ..Default::default()
                }) as Box<dyn ValExpression>)
            })
            .collect::<Result<Vec<_>>>()
    }
}
