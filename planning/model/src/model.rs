use derive_more::derive::Display;

use crate::*;

pub struct Model {
    pub types: Types,
    pub objects: Objects,
    pub fluents: Fluents,
    pub goals: Vec<Goal>,
}

impl Model {
    pub fn new(types: Types, objects: Objects, fluents: Fluents) -> Self {
        Self {
            types,
            objects,
            fluents,
            goals: Default::default(),
        }
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}\n", self.objects, self.fluents)?;
        write!(f, "\nGoals:")?;
        for g in &self.goals {
            write!(f, "\n  {g}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Display)]
#[display("[{}] {}", timestamp, goal)]
pub struct Goal {
    pub timestamp: Timestamp,
    pub goal: TypedExpr,
}

impl Goal {
    pub fn at_horizon(goal: TypedExpr) -> Self {
        Self {
            timestamp: Timestamp::HORIZON,
            goal,
        }
    }
}
