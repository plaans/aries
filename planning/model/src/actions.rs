use itertools::Itertools;
use std::collections::BTreeMap;

use thiserror::Error;

use crate::*;

#[derive(Error, Debug)]
pub enum ActionsError {
    #[error("Duplicate action")]
    DuplicateAction(Sym, Sym),
    #[error("Unknown action")]
    UnkonwnAction(Sym),
}

#[derive(Default)]
pub struct Actions {
    actions: BTreeMap<Sym, Action>,
}

impl Actions {
    pub fn add(&mut self, action: Action) -> Result<(), ActionsError> {
        if let Some(prev) = self.actions.get(&action.name) {
            return Err(ActionsError::DuplicateAction(action.name, prev.name.clone()));
        }
        self.actions.insert(action.name.clone(), action);
        Ok(())
    }
    pub fn iter(&self) -> impl Iterator<Item = &Action> {
        self.actions.values()
    }
}

#[derive(Debug, Clone)]
pub enum Duration {
    Instantaneous,
    Fixed(TypedExpr),
    Bounded(TypedExpr, TypedExpr),
}

#[derive(Debug)]
pub struct Action {
    pub name: Sym,
    pub parameters: Vec<Param>,
    pub duration: Duration,
    pub conditions: Vec<Condition>,
    pub effects: Vec<Effect>,
}

impl Action {
    pub fn new(name: impl Into<Sym>, parameters: Vec<Param>, duration: Duration) -> Self {
        Self {
            name: name.into(),
            parameters,
            duration,
            conditions: Default::default(),
            effects: Default::default(),
        }
    }
    pub fn instantaneous(name: impl Into<Sym>, parameters: Vec<Param>) -> Self {
        Self::new(name, parameters, Duration::Instantaneous)
    }

    pub fn start(&self) -> TimeRef {
        TimeRef::Start
    }

    pub fn end(&self) -> TimeRef {
        TimeRef::End
    }

    pub fn span(&self) -> TimeInterval {
        TimeInterval::closed(self.start(), self.end())
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({})",
            self.name,
            self.parameters
                .iter()
                .map(|p| format!("{}: {}", p.name(), p.tpe()))
                .format(", ")
        )?;
        write!(f, "\n    duration: {:?}", self.duration)?;
        write!(f, "\n    conditions:")?;
        for c in &self.conditions {
            write!(f, "\n      {c}")?;
        }
        write!(f, "\n    effects:")?;
        for eff in &self.effects {
            write!(f, "\n      {eff}")?;
        }
        Ok(())
    }
}
