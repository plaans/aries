use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use aries_bench_data::IntermediateResult;
use aries_solver::{prelude::*, solver::Exit};
use clap::Parser;
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

    /// Maximum runtime in seconds.
    #[arg(short, long)]
    pub timeout: Option<u32>,

    /// If set, a summary of the run will be saved in the indicated directory.
    /// This option is intended to ease the collection of benchmark results with `aries-bench`
    #[arg(short, long)]
    report: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Window {
    pub start: IntCst,
    pub end: IntCst,
}

impl Window {
    pub fn duration(&self) -> IntCst {
        self.end - self.start
    }

    pub fn is_before(&self, other: &Window) -> bool {
        self.end <= other.start
    }

    pub fn is_after(&self, other: &Window) -> bool {
        self.start >= other.end
    }

    pub fn is_ordered(&self, other: &Window) -> bool {
        self.is_before(other) || self.is_after(other)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnergyDelta {
    pub delta: IntCst,
    pub min: IntCst,
    pub max: IntCst,
}

impl EnergyDelta {
    pub fn enforce_if(
        &self,
        model: &mut Model,
        enabler: Lit,
        energy_at_start: Var,
        energy_at_end: Var,
        energy_max: IntCst,
    ) {
        // If delta is bigger than energy max there is no constraint
        // Otherwise: enabler -> energy_at_start + delta >= energy_at_start_j
        if self.delta < energy_max {
            model.enforce_if(enabler, geq(energy_at_start + self.delta, energy_at_end));
        }

        // Delta min constraint is implied by delta constraint if delta_min <= -delta
        // Otherwise: enabler -> energy_at_start >= delta_min
        if self.min > 0 && self.min > -self.delta {
            model.enforce_if(enabler, geq(energy_at_start, self.min));
        }

        // Delta max constraint is implied by delta constraint if delta_max <= -delta
        // Otherwise: enabler -> energy_at_start <= delta_max
        if self.max > 0 && self.max > -self.delta {
            model.enforce_if(enabler, leq(energy_at_end, energy_max - self.max));
        }
    }

    pub fn enforce_scoped(
        &self,
        model: &mut Model,
        scope: impl IntoIterator<Item = Lit> + Clone,
        energy_at_start: Var,
        energy_at_end: Var,
        energy_max: IntCst,
    ) {
        // If delta is bigger than energy max there is no constraint
        // Otherwise: scope -> energy_at_start + delta >= energy_at_start_j
        if self.delta < energy_max {
            model.enforce_scoped(geq(energy_at_start + self.delta, energy_at_end), scope.clone());
        }

        // Delta min constraint is implied by delta constraint if delta_min <= -delta
        // Otherwise: scope -> energy_at_start >= delta_min
        if self.min > 0 && self.min > -self.delta {
            model.enforce_scoped(geq(energy_at_start, self.min), scope.clone());
        }

        // Delta max constraint is implied by delta constraint if delta_max <= -delta
        // Otherwise: scope -> energy_at_start <= delta_max
        if self.max > 0 && self.max > -self.delta {
            model.enforce_scoped(leq(energy_at_end, energy_max - self.max), scope);
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Label {
    pub duration: IntCst,
    pub energy: EnergyDelta,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskJson {
    pub window: Window,
    pub label: Label,
    pub optional: bool,
    pub reward: IntCst,
}

#[derive(Debug)]
pub struct Task {
    pub id: usize,
    pub window: Window,
    pub label: Label,
    pub optional: bool,
    pub reward: IntCst,
}

impl Task {
    pub fn duration(&self) -> IntCst {
        self.label.duration
    }

    pub fn energy(&self) -> &EnergyDelta {
        &self.label.energy
    }

    pub fn earliest_start_time(&self) -> IntCst {
        self.window.start
    }

    pub fn latest_end_time(&self) -> IntCst {
        self.window.end
    }

    pub fn earliest_end_time(&self) -> IntCst {
        self.earliest_start_time() + self.duration()
    }

    pub fn latest_start_time(&self) -> IntCst {
        self.latest_end_time() - self.duration()
    }
}

impl From<TaskJson> for Task {
    fn from(task_json: TaskJson) -> Self {
        Self {
            id: 0,
            window: task_json.window,
            label: task_json.label,
            optional: task_json.optional,
            reward: task_json.reward,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Transition {
    pub source: usize,
    pub destination: usize,
    pub label: Option<Label>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceJson {
    pub tasks: Vec<TaskJson>,
    pub transitions: Vec<Transition>,
    pub energy_max: IntCst,
}

impl InstanceJson {
    pub fn read(file: &PathBuf) -> Self {
        let content = fs::read_to_string(file).expect("error while reading file");
        serde_json::from_str(content.as_str()).expect("error while parsing json")
    }
}

#[derive(Debug)]
pub struct Instance {
    pub tasks: Vec<Task>,
    pub transitions: HashMap<(usize, usize), Option<Label>>,
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

        Self {
            tasks,
            transitions: instance_json
                .transitions
                .into_iter()
                .map(|t| ((t.source, t.destination), t.label))
                .collect(),
            energy_max: instance_json.energy_max,
        }
    }
}

pub struct TaskVar {
    pub id: usize,
    pub duration: IntCst,
    pub reward: IntCst,
    pub presence: Lit,
    pub start: Var,
    pub energy_at_start: Var,
    pub energy_at_end: Var,
}

impl TaskVar {
    pub fn new(model: &mut Model, task: &Task, energy_max: IntCst) -> Self {
        let presence = if task.optional {
            model.new_bvar(format!("presence_{}", task.id)).true_lit()
        } else {
            Lit::TRUE
        };

        let start = model.new_optional_ivar(
            task.earliest_start_time(),
            task.latest_start_time(),
            presence,
            format!("start_{}", task.id),
        );

        let energy_at_start = model.new_optional_ivar(0, energy_max, presence, format!("energy_at_start_{}", task.id));
        let energy_at_end = model.new_optional_ivar(0, energy_max, presence, format!("energy_at_end_{}", task.id));

        task.energy()
            .enforce_scoped(model, [presence], energy_at_start, energy_at_end, energy_max);

        Self {
            id: task.id,
            duration: task.duration(),
            reward: task.reward,
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

pub struct Encoding {
    pub num_observations: Var,
    pub task_vars: Vec<TaskVar>,
}

impl Encoding {
    pub fn new(model: &mut Model, instance: &Instance) -> Encoding {
        let task_vars: Vec<TaskVar> = instance
            .tasks
            .iter()
            .map(|task| TaskVar::new(model, task, instance.energy_max))
            .collect();

        let num_observations = model.new_variable(0, instance.num_tasks().try_into().unwrap());
        let mut sum = LinSum::zero();
        for task_var in &task_vars {
            if task_var.presence == Lit::TRUE {
                sum += task_var.reward;
            } else {
                sum += task_var.reward * task_var.presence.variable();
            }
        }
        model.enforce(sum.geq(num_observations));

        for (i, task_var_i) in task_vars.iter().enumerate() {
            for task_var_j in &task_vars[i + 1..] {
                let j = task_var_j.id;

                // Get a new presence variable presence_ij <=> (presence_i && presence_j)
                // This variable will be true when both tasks are present
                let presence_ij = model.get_conjunctive_scope(&[task_var_i.presence, task_var_j.presence]);

                // Create a decision variable with scope presence_ij.
                // When presence_ij is true, a boolean value will need to be assigned to this one.
                // The value will decide which of task i or task j precedes the other.
                let before_ij = model.new_optional_bool_var(presence_ij);

                let before_ji = before_ij.not();

                let label_ij = instance.transitions.get(&(i, j)).unwrap();
                let label_ji = instance.transitions.get(&(j, i)).unwrap();

                match label_ij {
                    Some(label_ij) => {
                        // before_ij => end_i + duration_ij <= start_j
                        model.enforce_if(before_ij, leq(task_var_i.end() + label_ij.duration, task_var_j.start));

                        label_ij.energy.enforce_if(
                            model,
                            before_ij,
                            task_var_i.energy_at_end,
                            task_var_j.energy_at_start,
                            instance.energy_max,
                        );
                    }
                    None => model.enforce_if(before_ij, Lit::FALSE),
                }

                // TODO: clean this copy-paste
                match label_ji {
                    Some(label_ji) => {
                        // before_ji => end_j + duration_ji <= start_i
                        model.enforce_if(before_ji, leq(task_var_j.end() + label_ji.duration, task_var_i.start));

                        label_ji.energy.enforce_if(
                            model,
                            before_ji,
                            task_var_j.energy_at_end,
                            task_var_i.energy_at_start,
                            instance.energy_max,
                        );
                    }
                    None => model.enforce_if(before_ji, Lit::FALSE),
                }
            }
        }

        Encoding {
            num_observations,
            task_vars,
        }
    }
}

fn solve(instance_file: &PathBuf, args: &Args) -> anyhow::Result<()> {
    let deadline = args
        .timeout
        .map(|duration| SearchLimit::Deadline(Instant::now() + Duration::from_secs(duration as u64)))
        .unwrap_or(SearchLimit::None);

    let start_time = std::time::Instant::now();

    let instance = Instance::read(instance_file);

    // let lower_bound = (args.lower_bound.unwrap_or(0)).max(pb.makespan_lower_bound() as u32);
    // println!("Initial lower bound: {lower_bound}");

    let mut model = Model::new();

    let encoding = Encoding::new(&mut model, &instance);

    let mut solver = Solver::new(model);
    // let mut solver = search::get_solver(solver, &args.search, &encoding);
    let mut solution_history: Vec<IntermediateResult> = Default::default();

    let mut best = Option::None;
    let result = solver.maximize_with_callback(
        encoding.num_observations,
        |obj, sol| {
            println!("New solution with objective: {}", obj);
            solution_history.push(IntermediateResult {
                timestamp: start_time.elapsed(),
                objective: obj as i64,
            });
            best = Some(sol.clone());
        },
        deadline,
    );
    solver.print_stats();
    println!();

    let status = match result {
        Ok(Some((obj, solution))) => {
            let optimum = solution.value_of(encoding.num_observations).unwrap();
            assert_eq!(obj, optimum); // sanity check
            println!("> OPTIMAL (cost: {optimum})");

            // if let Some(expected) = args.expected_makespan {
            //     assert_eq!(
            //         optimum as u32, expected,
            //         "The makespan found ({optimum}) is not the expected one ({expected})"
            //     );
            // };
            println!(
                "XX\t{}\t{}\t{}",
                instance_file.to_str().unwrap(),
                optimum,
                start_time.elapsed().as_secs_f64()
            );
            aries_bench_data::SolveStatus::SolvedOpt
        }
        Ok(None) => {
            println!("> UNSATISFIABLE");
            // assert!(args.expected_makespan.is_none(), "Expected a valid solution");
            aries_bench_data::SolveStatus::SolvedUnsat
        }
        Err(Exit::Interrupted) => match best.as_ref() {
            Some(sol) => {
                let best_cost = sol.value_of(encoding.num_observations).unwrap();
                println!("> TIMEOUT (best solution cost {best_cost})");
                aries_bench_data::SolveStatus::Timeout
            }
            None => {
                println!("> TIMEOUT (no solution found)");
                aries_bench_data::SolveStatus::Timeout
            }
        },
    };

    // if let Some(solution) = best.as_ref() {
    //     // export the solution to file if specified
    //     export(solution, &pb, &encoding, args.output.as_ref());
    // }

    if let Some(report_dir) = args.report.as_ref() {
        let problem = aries_bench_data::Problem {
            name: instance_file.to_str().unwrap().into(),
            timeout: args
                .timeout
                .map(|t| Duration::from_secs(t as u64))
                .unwrap_or(Duration::MAX),
            flags: Default::default(),
        };

        // if let Some(lb) = args.lower_bound {
        //     problem.flags.insert("lb".to_string(), lb.to_string());
        // }

        // if let Some(ub) = args.upper_bound {
        //     problem.flags.insert("ub".to_string(), ub.to_string());
        // }

        let result = aries_bench_data::SolveResult {
            problem,
            status,
            runtime: start_time.elapsed(),
            objective_value: best.map(|sol| sol.value_of(encoding.num_observations).unwrap() as i64),
            metrics: Default::default(),
            objective_history: solution_history,
        }
        .with_metric(
            aries_bench_data::SolverMetric::NumConflicts,
            solver.stats.num_conflicts as f64,
        )
        .with_metric(
            aries_bench_data::SolverMetric::NumDecisions,
            solver.stats.num_decisions as f64,
        )
        .with_metric(
            aries_bench_data::SolverMetric::NumDomUpdates,
            solver.stats.num_dom_updates as f64,
        );

        result.save_to_dir(report_dir.to_str().unwrap())?;
    }

    println!("TOTAL RUNTIME: {:.6}", start_time.elapsed().as_secs_f64());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();

    let args = Args::parse();

    solve(&args.instance, &args)

    // let instance = Instance::read(&args.instance);

    // // println!("{:?}", instance);
    // // println!("----------");

    // let mut model = Model::new();
    // let encoding = Encoding::new(&mut model, &instance);

    // println!("----------");

    // let mut solver = Solver::new(model);
    // let result = solver.maximize(encoding.num_observations, SearchLimit::None);

    // let (objective, solution) = result.expect("error while solving").expect("unsat");

    // for task_var in &encoding.task_vars {
    //     let present = solution.entails(task_var.presence);
    //     if present {
    //         let start = solution.lb(task_var.start);
    //         let end = start + task_var.duration;

    //         // Solution already set all variables to lower bound
    //         // TODO: find a way to use upper bound
    //         let energy_at_start = solution.ub(task_var.energy_at_start);
    //         let energy_at_end = solution.ub(task_var.energy_at_end);

    //         println!(
    //             "T{} [{} - {}] ({} - {})",
    //             task_var.id, start, end, energy_at_start, energy_at_end
    //         );
    //     } else {
    //         println!("T{} *", task_var.id);
    //     }
    // }

    // println!("----------");
    // println!("Objective: {objective}");
    // println!("Duration: {}", solver.stats.solve_time.as_secs_f64());

    // Ok(())
}
