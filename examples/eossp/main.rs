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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct EnergyDelta {
    pub delta: Option<IntCst>,
    pub min: Option<IntCst>,
    pub max: Option<IntCst>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskJson {
    pub release: IntCst,
    pub deadline: IntCst,
    pub duration: IntCst,
    pub energy: EnergyDelta,
    pub optional: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransitionJson {
    pub source: usize,
    pub destination: usize,
    pub duration: Option<IntCst>,
    pub energy: Option<EnergyDelta>,
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
    pub energy: EnergyDelta,
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

#[derive(Debug, Clone, Copy)]
pub struct TransitionLabel {
    pub duration: Option<IntCst>,
    pub energy: Option<EnergyDelta>,
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
    pub id: usize,
    pub duration: IntCst,
    pub presence: Lit,
    pub start: Var,
    pub energy_at_start: Var,
    pub energy_at_end: Var, // We could remove this variable by putting all saturation on energy at start
}

impl TaskVar {
    pub fn new(model: &mut Model, task: &Task, energy_max: IntCst) -> Self {
        let presence = if task.optional {
            model.new_bvar(format!("presence_{}", task.id)).true_lit()
        } else {
            Lit::TRUE
        };

        let latest_start = task.deadline - task.duration;
        let start = model.new_optional_ivar(task.release, latest_start, presence, format!("start_{}", task.id));

        let energy_at_start = model.new_optional_ivar(0, energy_max, presence, format!("energy_at_start_{}", task.id));
        let energy_at_end = model.new_optional_ivar(0, energy_max, presence, format!("energy_at_end_{}", task.id));

        // presence => energy_at_start + delta >= energy_at_end
        if let Some(delta) = task.energy.delta {
            model.enforce_scoped(geq(energy_at_start + delta, energy_at_end), [presence]);
        }

        // presence => energy_at_start >= delta_min
        if let Some(delta_min) = task.energy.min {
            model.enforce_scoped(geq(energy_at_start, delta_min), [presence]);
        }

        // presence => energy_at_end <= enrgy_max - delta_max
        if let Some(delta_max) = task.energy.max {
            model.enforce_scoped(leq(energy_at_end, energy_max - delta_max), [presence]);
        }

        Self {
            id: task.id,
            duration: task.duration,
            presence,
            start,
            energy_at_start,
            energy_at_end,
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
        energy_max: IntCst,
    ) -> Self {
        // Both duration and energy are none: i and j are incompatible
        if transition.label.duration.is_none() && transition.label.energy.is_none() {
            let before = Lit::FALSE;
            return Self { transition, before };
        }

        let presence = model.get_conjunctive_scope(&[task_var_i.presence, task_var_j.presence]);

        let before = model
            .new_optional_bvar(presence, format!("before_{}_{}", task_var_i.id, task_var_j.id))
            .true_lit();

        // before_ij => end_i + duration_ij <= start_j
        if let Some(duration) = transition.label.duration {
            model.enforce_if(before, leq(task_var_i.end() + duration, task_var_j.start));
        }

        if let Some(energy) = transition.label.energy {
            // before_ij -> energy_at_end_i + delta_ij >= energy_at_start_j
            if let Some(delta) = energy.delta {
                model.enforce_if(
                    before,
                    geq(task_var_i.energy_at_end + delta, task_var_j.energy_at_start),
                );
            }

            // before_ij -> energy_at_end_i >= delta_min_ij
            if let Some(delta_min) = energy.delta {
                model.enforce_if(before, geq(task_var_i.energy_at_end, delta_min));
            }

            // before_ij -> energy_at_start_j <= energy_max - delta_max_ij
            if let Some(delta_max) = energy.delta {
                model.enforce_if(before, leq(task_var_j.energy_at_start, energy_max - delta_max));
            }
        }

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
            .map(|task| TaskVar::new(model, task, instance.energy_max))
            .collect();

        let transition_vars = instance
            .transitions
            .iter()
            .map(|((i, j), t)| {
                TransitionVar::new(
                    model,
                    ((*i, *j), *t).into(),
                    &task_vars[*i],
                    &task_vars[*j],
                    instance.energy_max,
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

#[derive(Debug)]
struct TaskSol {
    pub id: usize,
    pub start: IntCst,
    pub duration: IntCst,
    pub energy_at_start: IntCst,
    pub energy_at_end: IntCst,
}

impl TaskSol {
    pub fn new(solution: &Solution, task_var: &TaskVar) -> Self {
        Self {
            id: task_var.id,
            start: solution.lb(task_var.start),
            duration: task_var.duration,
            energy_at_start: solution.ub(task_var.energy_at_start),
            energy_at_end: solution.ub(task_var.energy_at_end),
        }
    }

    pub fn end(&self) -> IntCst {
        self.start + self.duration
    }

    pub fn energy(&self) -> IntCst {
        self.energy_at_end - self.energy_at_start
    }
}

#[derive(Debug)]
struct EosspSolution {
    pub task_sols: Vec<TaskSol>,
}

impl EosspSolution {
    pub fn new(solution: &Solution, model: &EosspModel) -> Self {
        let mut present_task_vars: Vec<&TaskVar> = model
            .task_vars
            .iter()
            .filter(|task_var| solution.entails(task_var.presence))
            .collect();

        present_task_vars.sort_by_key(|task_var| solution.lb(task_var.start));

        Self {
            task_sols: present_task_vars
                .iter()
                .map(|task_var| TaskSol::new(solution, task_var))
                .collect(),
        }
    }
}

fn check(solution: &EosspSolution, instance: &Instance) {
    for task_sol in &solution.task_sols {
        let task = &instance.tasks[task_sol.id];

        ma::assert_ge!(task_sol.start, task.release);
        ma::assert_le!(task_sol.end(), task.deadline);

        ma::assert_ge!(task_sol.energy_at_start, 0);
        ma::assert_le!(task_sol.energy_at_start, instance.energy_max);

        ma::assert_ge!(task_sol.energy_at_end, 0);
        ma::assert_le!(task_sol.energy_at_end, instance.energy_max);

        assert_eq!(task_sol.duration, task.duration);
        if let Some(delta) = task.energy.delta {
            ma::assert_le!(task_sol.energy(), delta);
        }
        if let Some(delta_min) = task.energy.min {
            ma::assert_ge!(task_sol.energy_at_start, delta_min);
        }
        if let Some(delta_max) = task.energy.max {
            ma::assert_le!(task_sol.energy_at_end, instance.energy_max - delta_max);
        }
    }

    // Check transitions
    for (task_sol_i, task_sol_j) in solution.task_sols.iter().zip(solution.task_sols.iter().skip(1)) {
        let i = task_sol_i.id;
        let j = task_sol_j.id;

        if let Some(transition) = instance.transitions.get(&(i, j)) {
            assert!(transition.duration.is_some() || transition.energy.is_some());

            if let Some(duration) = transition.duration {
                ma::assert_le!(task_sol_i.end() + duration, task_sol_j.start);
            }

            if let Some(energy) = transition.energy {
                if let Some(delta) = energy.delta {
                    ma::assert_ge!(task_sol_i.energy_at_end + delta, task_sol_j.energy_at_start,);
                }
                if let Some(delta_min) = energy.min {
                    ma::assert_ge!(task_sol_i.energy_at_end, delta_min);
                }
                if let Some(delta_max) = energy.max {
                    ma::assert_le!(task_sol_j.energy_at_start, instance.energy_max - delta_max);
                }
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let instance = Instance::read(&args.instance);

    println!("{:?}", instance);
    println!("----------");

    let mut aries_model = Model::new();

    let model = EosspModel::new(&mut aries_model, &instance);

    let mut solver = Solver::new(aries_model);
    let result = solver.maximize(model.objective, SearchLimit::None);

    let (objective, solution) = result.expect("error while solving").expect("unsat");

    for task_var in model.task_vars.iter() {
        let present = solution.entails(task_var.presence);
        if present {
            let start = solution.lb(task_var.start);
            let end = start + task_var.duration;

            // Solution already set all variables to lower bound
            // TODO: find a way to use upper bound
            let energy_at_start = solution.ub(task_var.energy_at_start);
            let energy_at_end = solution.ub(task_var.energy_at_end);

            println!(
                "T{} [{} - {}] ({} - {})",
                task_var.id, start, end, energy_at_start, energy_at_end
            );
        } else {
            println!("T{} *", task_var.id);
        }
    }

    println!("----------");

    let solution = EosspSolution::new(&solution, &model);

    println!("{:?}", solution);
    println!("Objective: {objective}");
    println!("----------");

    check(&solution, &instance);

    Ok(())
}
