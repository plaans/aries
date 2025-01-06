use crate::*;

pub enum Duration {
    Instantaneous,
}

pub struct Action {
    pub name: Sym,
    pub parameters: Vec<Param>,
    pub duration: Duration,
    // conditions: Condition,
    pub effects: Vec<Effect>,
}

impl Action {
    pub fn instantaneous(name: impl Into<Sym>, parameters: Vec<Param>) -> Self {
        Self {
            name: name.into(),
            parameters,
            duration: Duration::Instantaneous,
            effects: vec![],
        }
    }
}
