use crate::{env::Environment, *};

mod conditions;
pub use conditions::*;

pub struct Model {
    pub env: Environment,
    pub actions: Actions,
    /// Set of effects in the problem definition, covering both initial effects (at ORIGIN) and
    /// timed effects (after ORIGIN)
    pub init: Vec<Effect>,
    pub goals: Vec<Goal>,
    pub preferences: Preferences<Goal>,
    pub metric: Option<Metric>,
}

impl Model {
    pub fn new(types: Types) -> Self {
        Self {
            env: Environment::new(types),
            actions: Default::default(),
            init: Default::default(),
            goals: Default::default(),
            preferences: Default::default(),
            metric: Default::default(),
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
        write!(f, "\n\nPreferences:")?;
        for g in self.preferences.iter() {
            write!(f, "\n  {}", &self.env / g)?;
        }
        if let Some(metric) = &self.metric {
            write!(f, "\n\nMetric:\n  {}", &self.env / metric)?;
        }
        Ok(())
    }
}
