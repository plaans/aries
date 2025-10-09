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
    pub task_network: Option<TaskNet>,
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
            task_network: None,
        }
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}\n", self.env.objects, self.env.fluents)?;

        fstop(f, "Actions:", true, self.actions.iter(), &self.env)?;
        fstop(f, "Init:", false, &self.init, &self.env)?;
        fstop(f, "Goals:", false, &self.goals, &self.env)?;
        fstop(f, "Preferences:", false, self.preferences.iter(), &self.env)?;
        fstop(f, "Metric:", false, self.metric.iter(), &self.env)?;

        if let Some(tn) = &self.task_network {
            fstop(f, "Variables:", false, &tn.variables, &self.env)?;
            fstop(f, "Tasks:", false, tn.iter(), &self.env)?;
            fstop(f, "Constraints:", false, tn.constraints.iter().copied(), &self.env)?;
        }
        Ok(())
    }
}
