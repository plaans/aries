use std::{collections::HashMap, fs, path::PathBuf};

use aries::{
    core::{IntCst, Lit},
    model::lang::{IAtom, IVar},
    solver::Solver,
};
use clap::Parser;
use serde::{Deserialize, Serialize};

type AriesModel = aries::model::Model<String>;

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

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskJson {
    pub release: IntCst,
    pub deadline: IntCst,
    pub duration: IntCst,
    pub energy: IntCst,
    pub optional: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransitionJson {
    pub source: usize,
    pub destination: usize,
    pub duration: Option<IntCst>,
    pub energy: Option<IntCst>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceJson {
    pub tasks: Vec<TaskJson>,
    pub transitions: Vec<TransitionJson>,
}

impl InstanceJson {
    pub fn read(file: &PathBuf) -> Self {
        let content = fs::read_to_string(file).expect("error while reading file");
        serde_json::from_str(content.as_str()).expect("error while parsing json")
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: usize,
    pub release: IntCst,
    pub deadline: IntCst,
    pub duration: IntCst,
    pub energy: IntCst,
    pub optional: bool,
}

impl From<TaskJson> for Task {
    fn from(value: TaskJson) -> Self {
        Self {
            id: 0,
            release: value.release,
            deadline: value.deadline,
            duration: value.duration,
            energy: value.energy,
            optional: value.optional,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransitionGraph {
    pub edges: HashMap<(usize, usize), (Option<IntCst>, Option<IntCst>)>,
}

impl From<Vec<TransitionJson>> for TransitionGraph {
    fn from(transition_jsons: Vec<TransitionJson>) -> Self {
        let edges = transition_jsons
            .iter()
            .map(|t| ((t.source, t.destination), (t.duration, t.energy)))
            .collect();
        Self { edges }
    }
}

#[derive(Debug)]
pub struct Instance {
    pub tasks: Vec<Task>,
    pub transitions: TransitionGraph,
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
        let tasks = instance_json.tasks.into_iter().map(Into::into).collect();
        let transitions = instance_json.transitions.into();
        Self { tasks, transitions }
    }
}

pub struct TaskVar {
    pub task: Task,
    pub presence: Lit,
    pub start: IVar,
    pub end: IAtom,
}

impl TaskVar {
    pub fn new(model: &mut AriesModel, task: Task) -> Self {
        let presence = if task.optional {
            model.new_bvar(format!("p_{}", task.id)).into()
        } else {
            Lit::TRUE
        };

        let lates_start = task.deadline - task.duration;
        let start = model.new_optional_ivar(task.release, lates_start, presence, format!("s_{}", task.id));

        let end = start + task.duration;

        Self {
            task,
            presence,
            start,
            end,
        }
    }
}

pub struct Model {
    pub model: AriesModel,
    pub task_vars: Vec<TaskVar>,
    pub transitions: TransitionGraph,
    pub objective: IVar,
}

impl Model {
    pub fn new(instance: &Instance) -> Self {
        let mut model = AriesModel::new();
        let task_vars = instance
            .tasks
            .iter()
            .map(|task| TaskVar::new(&mut model, task.clone()))
            .collect();

        let transitions = instance.transitions.clone();

        let num_tasks: i32 = instance.num_tasks().try_into().unwrap();
        let objective = model.new_ivar(0, num_tasks, "objective");

        Self {
            model,
            task_vars,
            transitions,
            objective,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let instance = Instance::read(&args.instance);
    let model = Model::new(&instance);

    println!("{:?}", instance);
    model.model.print_state();

    let mut solver = Solver::new(model.model);
    let result = solver.maximize(model.objective);

    let (objective, domains) = result.unwrap().unwrap();
    println!("{objective}");

    Ok(())
}
