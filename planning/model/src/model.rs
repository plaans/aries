use crate::{env::Environment, *};

mod conditions;
pub use conditions::*;

pub struct Model {
    pub env: Environment,
    pub actions: Actions,
    /// Set of effects in the problem definition, covering both initial effects (at ORIGIN) and
    /// timed effects (after ORIGIN)
    pub init: Vec<Effect>,
    pub goals: Vec<Condition>,
}

impl Model {
    pub fn new(types: Types, objects: Objects, fluents: Fluents) -> Self {
        Self {
            env: Environment::new(types, objects, fluents),
            actions: Default::default(),
            init: Default::default(),
            goals: Default::default(),
        }
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}\n", self.env.objects, self.env.fluents)?;

        write!(f, "\nActions:")?;
        for a in self.actions.iter() {
            write!(f, "\n\n  {}", &self.env / a)?;
        }
        write!(f, "\n\nInit:")?;
        for ini in &self.init {
            write!(f, "\n  {}", &self.env / ini)?;
        }

        write!(f, "\n\nGoals:")?;
        for g in &self.goals {
            write!(f, "\n  {}", &self.env / g)?;
        }
        Ok(())
    }
}
