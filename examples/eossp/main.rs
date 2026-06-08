use std::{fs, path::PathBuf};

use aries::{
    core::{IntCst, Lit},
    model::lang::{
        IAtom, IVar,
        expr::{eq, geq, leq, or},
        linear::LinearSum,
    },
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
    pub energy_max: IntCst,
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

impl Task {
    /// Return true iff task i must be before task j ie i < j.
    pub fn before(task_i: &Task, task_j: &Task, t_ji: IntCst) -> bool {
        return task_j.release + task_j.duration + t_ji + task_i.duration > task_i.deadline;
    }
}

impl From<TaskJson> for Task {
    fn from(task_json: TaskJson) -> Self {
        Self {
            id: 0,
            release: task_json.release,
            deadline: task_json.deadline,
            duration: task_json.duration,
            energy: task_json.energy,
            optional: task_json.optional,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Transition {
    pub source: usize,
    pub destination: usize,
    pub duration: Option<IntCst>,
    pub energy: Option<IntCst>,
}

impl From<TransitionJson> for Transition {
    fn from(transition_json: TransitionJson) -> Self {
        Self {
            source: transition_json.source,
            destination: transition_json.destination,
            duration: transition_json.duration,
            energy: transition_json.energy,
        }
    }
}

#[derive(Debug)]
pub struct Instance {
    pub tasks: Vec<Task>,
    pub transitions: Vec<Transition>,
    pub energy_max: IntCst,
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

        let transitions = instance_json.transitions.into_iter().map(Into::into).collect();
        let energy_max = instance_json.energy_max;

        Self {
            tasks,
            transitions,
            energy_max,
        }
    }
}

pub struct TaskVar {
    pub task: Task,
    pub presence: Lit,
    pub start: IVar,
    pub energy_at_start: IVar,
    pub energy_at_end: IVar, // We could remove this variable by putting all saturation on energy at start
}

impl TaskVar {
    pub fn new(model: &mut AriesModel, task: Task, energy_max: IntCst) -> Self {
        let presence = if task.optional {
            model.new_bvar(format!("p_{}", task.id)).true_lit()
        } else {
            Lit::TRUE
        };

        let latest_start = task.deadline - task.duration;
        let start = model.new_optional_ivar(task.release, latest_start, presence, format!("s_{}", task.id));

        let energy_at_start = model.new_optional_ivar(0, energy_max, presence, format!("a_{}", task.id));
        let energy_at_end = model.new_optional_ivar(0, energy_max, presence, format!("b_{}", task.id));
        model.enforce(eq(energy_at_start + task.energy, energy_at_end), [presence]);

        Self {
            task,
            presence,
            start,
            energy_at_start,
            energy_at_end,
        }
    }

    pub fn end(&self) -> IAtom {
        self.start + self.task.duration
    }

    /// Return literal b_ij such that b_ij => e_i + t_ij <= s_j.
    pub fn before(model: &mut AriesModel, task_i: &Self, task_j: &Self, t_ij: IntCst) -> Lit {
        model.half_reify(leq(task_i.end() + t_ij, task_j.start))
    }

    /// Post energy constraint b_ij => b_i + e_ij >= a_i.
    pub fn post_energy(model: &mut AriesModel, task_i: &Self, task_j: &Self, e_ij: IntCst, b_ij: Lit) {
        model.enforce_if(b_ij, geq(task_i.energy_at_end + e_ij, task_j.energy_at_start));
    }

    /// Post transition constraints for i -> j.
    pub fn post_transition(
        model: &mut AriesModel,
        task_i: &Self,
        task_j: &Self,
        duration: &Option<IntCst>,
        energy: &Option<IntCst>,
    ) {
        // No duration and no energy: i is uncompatible with j
        if duration.is_none() && energy.is_none() {
            model.enforce(or(vec![task_i.presence.not(), task_j.presence.not()]), []);
            return;
        }

        // Post temporal constraint
        let d = duration.unwrap_or(0);
        let i_before_j = Self::before(model, task_i, task_j, d);

        // Post energy constraint
        if let Some(e) = energy {
            Self::post_energy(model, task_i, task_j, *e, i_before_j);
        }
    }
}

pub struct Model {
    pub model: AriesModel,
    pub task_vars: Vec<TaskVar>,
    pub transitions: Vec<Transition>,
    pub objective: IVar,
}

impl Model {
    pub fn new(instance: &Instance) -> Self {
        let mut model = AriesModel::new();
        let task_vars = instance
            .tasks
            .iter()
            .map(|task| TaskVar::new(&mut model, task.clone(), instance.energy_max))
            .collect();

        let transitions = instance.transitions.clone();

        let num_tasks: i32 = instance.num_tasks().try_into().unwrap();
        let objective = model.new_ivar(0, num_tasks, "objective");

        let mut model = Self {
            model,
            task_vars,
            transitions,
            objective,
        };

        model.post_transitions();
        model.post_objective();
        model
    }

    fn post_transitions(&mut self) {
        for transition in self.transitions.iter() {
            let task_i = &self.task_vars[transition.source];
            let task_j = &self.task_vars[transition.destination];
            TaskVar::post_transition(
                &mut self.model,
                task_i,
                task_j,
                &transition.duration,
                &transition.energy,
            );
        }
    }

    fn post_objective(&mut self) {
        let mut sum = LinearSum::zero();
        for task in self.task_vars.iter() {
            if task.presence == Lit::TRUE {
                sum += 1;
            } else {
                sum += IVar::new(task.presence.variable());
            }
        }
        self.model.enforce(sum.clone().geq(self.objective), []);
        self.model.enforce(sum.leq(self.objective), []);
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let instance = Instance::read(&args.instance);
    let model = Model::new(&instance);

    println!("{:?}", instance);

    println!("----------");

    model.model.print_state();

    let mut solver = Solver::new(model.model);
    let result = solver.maximize(model.objective);

    let (objective, domains) = result.expect("error while solving").expect("unsat");

    println!("----------");

    for task_var in model.task_vars {
        let present = domains.entails(task_var.presence);
        if present {
            let start = domains.lb(task_var.start);
            let end = start + task_var.task.duration;

            // Domains already set all variables to lower bound
            // TODO: find a way to use upper bound
            let energy_at_start = domains.ub(task_var.energy_at_start);
            let energy_at_end = domains.ub(task_var.energy_at_end);

            println!(
                "T{} [{} - {}] ({} - {})",
                task_var.task.id, start, end, energy_at_start, energy_at_end
            );
        } else {
            println!("T{} *", task_var.task.id);
        }
    }

    println!("Objective: {objective}");

    Ok(())
}
