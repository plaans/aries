use crate::*;

mod conditions;
pub use conditions::*;

pub struct Model {
    pub types: Types,
    pub objects: Objects,
    pub fluents: Fluents,
    pub actions: Actions,
    /// Set of effects in the problem definition, covering both initial effects (at ORIGIN) and
    /// timed effects (after ORIGIN)
    pub init: Vec<Effect>,
    pub goals: Vec<Condition>,
}

impl Model {
    pub fn new(types: Types, objects: Objects, fluents: Fluents) -> Self {
        Self {
            types,
            objects,
            fluents,
            actions: Default::default(),
            init: Default::default(),
            goals: Default::default(),
        }
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}\n", self.objects, self.fluents)?;

        write!(f, "\nActions:")?;
        for a in self.actions.iter() {
            write!(f, "\n\n  {a}")?;
        }
        write!(f, "\n\nInit:")?;
        for ini in &self.init {
            write!(f, "\n  {ini}")?;
        }

        write!(f, "\n\nGoals:")?;
        for g in &self.goals {
            write!(f, "\n  {g}")?;
        }
        Ok(())
    }
}
