use std::{collections::HashMap, fs, path::PathBuf};

use aries::{
    core::{IntCst, Lit},
    model::lang::{
        IAtom, IVar,
        expr::{geq, leq},
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
        task_j.release + task_j.duration + t_ji + task_i.duration > task_i.deadline
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
pub struct TransitionLabel {
    pub duration: Option<IntCst>,
    pub energy: Option<IntCst>,
}

impl From<&TransitionJson> for (usize, usize) {
    fn from(transition_json: &TransitionJson) -> Self {
        (transition_json.source, transition_json.destination)
    }
}

impl From<&TransitionJson> for TransitionLabel {
    fn from(transition_json: &TransitionJson) -> Self {
        Self {
            duration: transition_json.duration,
            energy: transition_json.energy,
        }
    }
}

impl From<&TransitionJson> for ((usize, usize), TransitionLabel) {
    fn from(transition_json: &TransitionJson) -> Self {
        (transition_json.into(), transition_json.into())
    }
}

#[derive(Debug, Clone)]
pub struct Transition {
    pub source: usize,
    pub destination: usize,
    pub label: TransitionLabel,
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

impl From<((usize, usize), TransitionLabel)> for Transition {
    fn from(value: ((usize, usize), TransitionLabel)) -> Self {
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
    pub tasks: Vec<Task>,
    pub transitions: HashMap<(usize, usize), TransitionLabel>,
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

        let transitions = instance_json.transitions.iter().map(Into::into).collect();
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
            model.new_bvar(format!("presence_{}", task.id)).true_lit()
        } else {
            Lit::TRUE
        };

        let latest_start = task.deadline - task.duration;
        let start = model.new_optional_ivar(task.release, latest_start, presence, format!("start_{}", task.id));

        let energy_at_start = model.new_optional_ivar(0, energy_max, presence, format!("energy_at_start_{}", task.id));
        let energy_at_end = model.new_optional_ivar(0, energy_max, presence, format!("energy_at_end_{}", task.id));
        model.enforce(geq(energy_at_start + task.energy, energy_at_end), [presence]);

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
}

pub struct TransitionVar {
    pub transition: Transition,
    pub before: Lit,
}

impl TransitionVar {
    pub fn new(model: &mut AriesModel, transition: Transition, task_i: &TaskVar, task_j: &TaskVar) -> Self {
        // Both duration and energy are none: i and j are incompatible
        if transition.label.duration.is_none() && transition.label.energy.is_none() {
            let before = Lit::FALSE;
            return Self { transition, before };
        }

        // Problem: presence is optional even if both task_i.presence and task_j.presence are mandatory
        // let presence = model.reify(and([task_i.presence, task_j.presence]));

        let presence = model.new_bvar("").true_lit();

        // model.enforce(implies(presence, task_i.presence), []);
        // model.enforce(implies(presence, task_j.presence), []);
        // model.enforce(implies(task_i.presence, presence), []);
        // model.enforce(implies(task_j.presence, presence), []);

        let before = model
            .new_optional_bvar(presence, format!("before_{}_{}", task_i.task.id, task_j.task.id))
            .true_lit();

        // before_ij => end_i + duration_ij <= start_j
        if let Some(duration) = transition.label.duration {
            model.enforce_if(before, leq(task_i.end() + duration, task_j.start));
        }

        // before_ij => energy_at_end_i + energy_ij >= energy_at_start_j
        // if let Some(energy) = transition.label.energy {
        //     model.enforce_if(enabler, geq(task_i.energy_at_end + energy, task_j.energy_at_start));
        // }

        Self { transition, before }
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

pub struct Model {
    pub model: AriesModel,
    pub task_vars: Vec<TaskVar>,
    pub transition_vars: HashMap<(usize, usize), TransitionVar>,
    pub objective: IVar,
}

impl Model {
    pub fn new(instance: Instance) -> Self {
        let mut model = AriesModel::new();

        let num_tasks: IntCst = instance.num_tasks().try_into().expect("overflow on num_tasks");
        let objective = model.new_ivar(0, num_tasks, "objective");

        let task_vars: Vec<TaskVar> = instance
            .tasks
            .into_iter()
            .map(|task| TaskVar::new(&mut model, task, instance.energy_max))
            .collect();

        let transition_vars = instance
            .transitions
            .into_iter()
            .map(|((i, j), t)| TransitionVar::new(&mut model, ((i, j), t).into(), &task_vars[i], &task_vars[j]).into())
            .collect();

        let mut model = Self {
            model,
            task_vars,
            transition_vars,
            objective,
        };

        // model.post_disjunctions();
        model.post_objective();
        model
    }

    // fn post_disjunctions(&mut self) {
    //     for ((i, j), transition_ij) in self.transition_vars.iter() {
    //         let task_i = &self.task_vars[*i];
    //         let task_j = &self.task_vars[*j];

    //         let mut disjunction = vec![transition_ij.before.true_lit()];

    //         if let Some(transition_ji) = self.transition_vars.get(&(*j, *i)) {
    //             disjunction.push(transition_ji.before.true_lit());
    //         }

    //         self.model.enforce(or(disjunction), [task_i.presence, task_j.presence]);
    //     }
    // }

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
    println!("{:?}", instance);

    let model = Model::new(instance);

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
