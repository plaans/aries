use crate::*;

mod conditions;
pub use conditions::*;

pub struct Model {
    pub types: Types,
    pub objects: Objects,
    pub fluents: Fluents,
    pub actions: Actions,
    pub goals: Vec<Condition>,
}

impl Model {
    pub fn new(types: Types, objects: Objects, fluents: Fluents) -> Self {
        Self {
            types,
            objects,
            fluents,
            actions: Default::default(),
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

        write!(f, "\nGoals:")?;
        for g in &self.goals {
            write!(f, "\n  {g}")?;
        }
        Ok(())
    }
}
