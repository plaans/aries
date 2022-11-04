use anyhow::{Context, Result};

use crate::models::{condition::ValCondition, expression::ValExpression};

impl ValCondition for unified_planning::Condition {
    fn expr(&self) -> Result<Box<dyn ValExpression>> {
        let e = self.cond.as_ref().context("Condition without expression")?;
        Ok(Box::new(e.clone()) as Box<dyn ValExpression>)
    }
}
