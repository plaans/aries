use std::{collections::HashMap, fs, path::PathBuf};

use aries_solver::prelude::*;
use clap::Parser;
use more_asserts as ma;
use serde::{Deserialize, Serialize};

/// Command line arguments.
#[derive(Parser, Debug)]
#[command(
    version,
    about = "EOSSP solver using Aries.",
    long_about = None
)]
pub struct Args {
    /// EOSSP instance.
    #[arg(value_name = "FILE")]
    pub instance: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResourceDelta {
    pub delta: Option<IntCst>,
    pub min: Option<IntCst>,
    pub max: Option<IntCst>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TaskLabel {
    pub duration: IntCst,
    pub resources: Vec<ResourceDelta>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransitionLabel {
    pub duration: Option<IntCst>,
    pub resources: Vec<ResourceDelta>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskJson {
    pub window: IntItv,
    pub label: TaskLabel,
    pub optional: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransitionJson {
    pub source: usize,
    pub destination: usize,
    pub label: Option<TransitionLabel>,
}

impl From<&TransitionJson> for (usize, usize) {
    fn from(transition_json: &TransitionJson) -> Self {
        (transition_json.source, transition_json.destination)
    }
}

impl From<&TransitionJson> for Option<TransitionLabel> {
    fn from(transition_json: &TransitionJson) -> Self {
        transition_json.label.clone()
    }
}

impl From<&TransitionJson> for ((usize, usize), Option<TransitionLabel>) {
    fn from(transition_json: &TransitionJson) -> Self {
        (transition_json.into(), transition_json.into())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IntItv(pub IntCst, pub IntCst);

impl IntItv {
    pub fn release(&self) -> IntCst {
        self.0
    }

    pub fn deadline(&self) -> IntCst {
        self.1
    }

    pub fn lb(&self) -> IntCst {
        self.0
    }

    pub fn ub(&self) -> IntCst {
        self.1
    }

    pub fn min(&self) -> IntCst {
        self.0
    }

    pub fn max(&self) -> IntCst {
        self.1
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    Minimize,
    Maximize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Resource {
    pub name: String,
    pub objective: Objective,
    pub bounds: IntItv,
    pub saturate: bool,
}

impl Resource {
    /// For now it assume saturate is true
    pub fn enforce_scoped(
        &self,
        model: &mut Model,
        resource_at_start: Var,
        resource_at_end: Var,
        scope: impl IntoIterator<Item = Lit> + Clone,
        delta: &ResourceDelta,
    ) {
        if !self.saturate {
            todo!();
        };

        match self.objective {
            Objective::Minimize => {
                // presence -> resource_at_start + delta <= resource_at_end
                if let Some(delta) = delta.delta {
                    model.enforce_scoped(leq(resource_at_start + delta, resource_at_end), scope.clone());
                }
        
                // presence -> resource_at_start <= resource_max - delta_max
                if let Some(delta_max) = delta.max {
                    model.enforce_scoped(leq(resource_at_start, self.bounds.max() - delta_max), scope.clone());
                }
        
                // presence -> resource_at_end >= resource_min + delta_min
                if let Some(delta_min) = delta.min {
                    model.enforce_scoped(geq(resource_at_end, self.bounds.min() + delta_min), scope.clone());
                }
            },
            Objective::Maximize => {
                // presence -> resource_at_start + delta >= resource_at_end
                if let Some(delta) = delta.delta {
                    model.enforce_scoped(geq(resource_at_start + delta, resource_at_end), scope.clone());
                }
        
                // presence -> resource_at_start >= resource_min + delta_min
                if let Some(delta_min) = delta.min {
                    model.enforce_scoped(geq(resource_at_start, self.bounds.min() + delta_min), scope.clone());
                }
        
                // presence -> resource_at_end <= resource_max - delta_max
                if let Some(delta_max) = delta.max {
                    model.enforce_scoped(leq(resource_at_end, self.bounds.max() - delta_max), scope.clone());
                }
            },
        }
    }

    /// For now it assume saturate is true
    pub fn enforce_if(
        &self,
        model: &mut Model,
        resource_at_start: Var,
        resource_at_end: Var,
        literal: Lit,
        delta: &ResourceDelta,
    ) {
        if !self.saturate {
            todo!();
        };

        match self.objective {
            Objective::Minimize => {
                // presence -> resource_at_start + delta <= resource_at_end
                if let Some(delta) = delta.delta {
                    model.enforce_if(literal, leq(resource_at_start + delta, resource_at_end));
                }
        
                // presence -> resource_at_start <= resource_max - delta_max
                if let Some(delta_max) = delta.max {
                    model.enforce_if(literal, leq(resource_at_start, self.bounds.max() - delta_max));
                }
        
                // presence -> resource_at_end >= resource_min + delta_min
                if let Some(delta_min) = delta.min {
                    model.enforce_if(literal, geq(resource_at_end, self.bounds.min() + delta_min));
                }
            },
            Objective::Maximize => {
                // presence -> resource_at_start + delta >= resource_at_end
                if let Some(delta) = delta.delta {
                    model.enforce_if(literal, geq(resource_at_start + delta, resource_at_end));
                }
        
                // presence -> resource_at_start >= resource_min + delta_min
                if let Some(delta_min) = delta.min {
                    model.enforce_if(literal, geq(resource_at_start, self.bounds.min() + delta_min));
                }
        
                // presence -> resource_at_end <= resource_max - delta_max
                if let Some(delta_max) = delta.max {
                    model.enforce_if(literal, leq(resource_at_end, self.bounds.max() - delta_max));
                }
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceJson {
    pub resources: Vec<Resource>,
    pub tasks: Vec<TaskJson>,
    pub transitions: Vec<TransitionJson>,
}

impl InstanceJson {
    pub fn read(file: &PathBuf) -> Self {
        let content = fs::read_to_string(file).expect("error while reading file");
        serde_json::from_str(content.as_str()).expect("error while parsing json")
    }
}

#[derive(Debug)]
pub struct Task {
    pub id: usize,
    pub window: IntItv,
    pub label: TaskLabel,
    pub optional: bool,
}

impl From<TaskJson> for Task {
    fn from(task_json: TaskJson) -> Self {
        Self {
            id: 0,
            window: task_json.window,
            label: task_json.label,
            optional: task_json.optional,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Transition {
    pub source: usize,
    pub destination: usize,
    pub label: Option<TransitionLabel>,
}

impl From<&TransitionJson> for Transition {
    fn from(transition_json: &TransitionJson) -> Self {
        Self {
            source: transition_json.source,
            destination: transition_json.destination,
            label: transition_json.into(),
        }
    }
}

impl From<((usize, usize), Option<TransitionLabel>)> for Transition {
    fn from(value: ((usize, usize), std::option::Option<TransitionLabel>)) -> Self {
        let ((source, destination), label) = value;
        Self {
            source,
            destination,
            label,
        }
    }
}

#[derive(Debug)]
pub struct Instance {
    pub resources: Vec<Resource>,
    pub tasks: Vec<Task>,
    pub transitions: HashMap<(usize, usize), Option<TransitionLabel>>,
}

impl Instance {
    pub fn read(file: &PathBuf) -> Self {
        InstanceJson::read(file).into()
    }

    pub fn num_tasks(&self) -> usize {
        self.tasks.len()
    }
}

impl From<InstanceJson> for Instance {
    fn from(instance_json: InstanceJson) -> Self {
        let mut tasks: Vec<Task> = instance_json.tasks.into_iter().map(Into::into).collect();
        for (i, task) in tasks.iter_mut().enumerate() {
            task.id = i;
        }

        Self {
            resources: instance_json.resources,
            tasks,
            transitions: instance_json.transitions.iter().map(Into::into).collect(),
        }
    }
}

pub struct TaskVar {
    pub id: usize,
    pub duration: IntCst,
    pub presence: Lit,
    pub start: Var,
    pub resource_vars: Vec<(Var, Var)>,
}

impl TaskVar {
    pub fn new(model: &mut Model, task: &Task, resources: &Vec<Resource>) -> Self {
        let presence = if task.optional {
            model.new_bvar(format!("presence_{}", task.id)).true_lit()
        } else {
            Lit::TRUE
        };

        let latest_start = task.window.deadline() - task.label.duration;

        let start = model.new_optional_ivar(
            task.window.release(),
            latest_start,
            presence,
            format!("start_{}", task.id),
        );

        let create_resource_pair = |r: usize| -> (Var, Var) {
            let resource = &resources[r];
            let resource_delta = &task.label.resources[r];

            let resource_at_start = model.new_optional_ivar(
                resource.bounds.lb(),
                resource.bounds.ub(),
                presence,
                format!("{}_at_start_{}", resource.name, task.id),
            );

            let resource_at_end = model.new_optional_ivar(
                resource.bounds.lb(),
                resource.bounds.ub(),
                presence,
                format!("{}_at_start_{}", resource.name, task.id),
            );

            resource.enforce_scoped(model, resource_at_start, resource_at_end, [presence], resource_delta);
            (resource_at_start, resource_at_end)
        };

        Self {
            id: task.id,
            duration: task.label.duration,
            presence,
            start,
            resource_vars: (0..resources.len()).map(create_resource_pair).collect(),
        }
    }

    pub fn end(&self) -> VarCst {
        self.start + self.duration
    }
}

pub struct TransitionVar {
    pub transition: Transition,
    pub before: Lit,
}

impl TransitionVar {
    pub fn new(
        model: &mut Model,
        transition: Transition,
        task_var_i: &TaskVar,
        task_var_j: &TaskVar,
        resources: &Vec<Resource>,
    ) -> Self {
        match &transition.label {
            None => {
                return Self {
                    transition,
                    before: Lit::FALSE,
                };
            }
            Some(label) => {
                let presence = model.get_conjunctive_scope(&[task_var_i.presence, task_var_j.presence]);

                let before = model
                    .new_optional_bvar(presence, format!("before_{}_{}", task_var_i.id, task_var_j.id))
                    .true_lit();

                // before_ij -> end_i + duration_ij <= start_j
                if let Some(duration) = label.duration {
                    model.enforce_if(before, leq(task_var_i.end() + duration, task_var_j.start));
                }

                for r in 0..resources.len() {
                    let resource = &resources[r];
                    let delta = &label.resources[r];
                    let (_, resource_at_end_i) = task_var_i.resource_vars[r];
                    let (resource_at_start_j, _) = task_var_j.resource_vars[r];

                    resource.enforce_if(model, resource_at_end_i, resource_at_start_j, before, delta);
                }

                Self { transition, before }
            }
        }
    }
}

impl From<TransitionVar> for ((usize, usize), TransitionVar) {
    fn from(transition_var: TransitionVar) -> Self {
        (
            (transition_var.transition.source, transition_var.transition.destination),
            transition_var,
        )
    }
}

pub struct EosspModel {
    pub task_vars: Vec<TaskVar>,
    pub transition_vars: HashMap<(usize, usize), TransitionVar>,
    pub objective: Var,
}

impl EosspModel {
    pub fn new(model: &mut Model, instance: &Instance) -> Self {
        let num_tasks: IntCst = instance.num_tasks().try_into().expect("overflow on num_tasks");
        let objective = model.new_ivar(0, num_tasks, "objective");

        let task_vars: Vec<TaskVar> = instance
            .tasks
            .iter()
            .map(|task| TaskVar::new(model, task, &instance.resources))
            .collect();

        let transition_vars = instance
            .transitions
            .iter()
            .map(|((i, j), t)| {
                TransitionVar::new(
                    model,
                    ((*i, *j), t.clone()).into(),
                    &task_vars[*i],
                    &task_vars[*j],
                    &instance.resources,
                )
                .into()
            })
            .collect();

        let mut eossp_model = Self {
            task_vars,
            transition_vars,
            objective,
        };

        eossp_model.post_disjunctions(model);
        eossp_model.post_objective(model);
        eossp_model
    }

    fn post_disjunctions(&mut self, model: &mut Model) {
        for ((i, j), transition_ij) in self.transition_vars.iter() {
            let task_i = &self.task_vars[*i];
            let task_j = &self.task_vars[*j];

            let mut disjunction = vec![task_i.presence.not(), task_j.presence.not(), transition_ij.before];

            if let Some(transition_ji) = self.transition_vars.get(&(*j, *i)) {
                if i < j {
                    continue;
                };
                disjunction.push(transition_ji.before);
            }

            model.enforce(or(disjunction));
        }
    }

    fn post_objective(&mut self, model: &mut Model) {
        let mut sum = LinSum::zero();
        for task in self.task_vars.iter() {
            if task.presence == Lit::TRUE {
                sum += 1;
            } else {
                sum += task.presence.variable();
            }
        }
        model.enforce(sum.clone().geq(self.objective));
        model.enforce(sum.leq(self.objective));
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();

    let args = Args::parse();

    let instance = Instance::read(&args.instance);

    println!("{:?}", instance);
    println!("----------");

    let mut aries_model = Model::new();

    let eossp_model = EosspModel::new(&mut aries_model, &instance);
    println!("----------");

    aries_model.print_state();
    println!("----------");

    let mut solver = Solver::new(aries_model);
    let result = solver.maximize(eossp_model.objective, SearchLimit::None);

    let (objective, solution) = result.expect("error while solving").expect("unsat");

    for task_var in eossp_model.task_vars.iter() {
        print!("T{}", task_var.id,);

        let present = solution.entails(task_var.presence);

        if present {
            let start = solution.lb(task_var.start);
            let end = start + task_var.duration;

            print!(" [{start} - {end}]");

            for (resource_at_start, resource_at_end) in &task_var.resource_vars {
                let resource_at_start = solution.ub(resource_at_start);
                let resource_at_end = solution.ub(resource_at_end);

                print!(" ({resource_at_start} - {resource_at_end})");
            }
        } else {
            print!(" *");
        }
        println!("");
    }

    println!("Objective: {objective}");
    println!("----------");

    Ok(())
}
