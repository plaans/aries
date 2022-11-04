use anyhow::Result;

use crate::models::{action::ValAction, plan::ValPlan};

impl ValPlan for unified_planning::Plan {
    fn actions(&self) -> Result<Vec<Box<dyn ValAction>>> {
        Ok(self
            .actions
            .iter()
            .map(|a| Box::new(a.clone()) as Box<dyn ValAction>)
            .collect::<Vec<_>>())
    }
}
