use aries_backtrack::{Backtrack, DecLvl};
use aries_model::bounds::Lit;
use aries_model::extensions::AssignmentExt;
use aries_model::lang::expr::{leq, or};
use aries_model::lang::{IVar, VarRef};
use aries_model::Label;
use aries_solver::solver::search::activity::{ActivityBrancher, Heuristic};
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;
use aries_tnet::theory::{StnConfig, StnTheory};
use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::str::FromStr;
use structopt::StructOpt;

#[derive(Copy, Clone, Debug)]
pub enum Var {
    /// Zero constant / Temporal origin
    Zero,
    /// Variable representing the makespan (constrained to be after the end of tasks
    Makespan,
    /// Variable representing the start time of (job_number, task_number_in_job)
    Start(usize, usize),
    /// Variable representing a constraint that was reified (normally a precedence constraint)
    Reified,
}

impl Label for Var {
    fn zero() -> Self {
        Var::Zero
    }

    fn reified() -> Self {
        Var::Reified
    }
}

type Model = aries_model::Model<Var>;
type Solver = aries_solver::solver::Solver<Var>;
type ParSolver = aries_solver::parallel_solver::ParSolver<Var>;

#[derive(Clone, Debug)]
struct JobShop {
    pub num_jobs: usize,
    pub num_machines: usize,
    times: Vec<i32>,
    machines: Vec<usize>,
}

impl JobShop {
    pub fn op_id(&self, job: usize, op: usize) -> usize {
        job * self.num_machines + op
    }
    pub fn tvar(&self, job: usize, op: usize) -> TVar {
        TVar(self.op_id(job, op) + 2)
    }
    pub fn duration(&self, job: usize, op: usize) -> i32 {
        self.times[job * self.num_machines + op]
    }
    pub fn machines(&self) -> impl Iterator<Item = usize> {
        1..=self.num_machines
    }
    pub fn jobs(&self) -> impl Iterator<Item = usize> {
        0..self.num_jobs
    }

    pub fn machine(&self, job: usize, op: usize) -> usize {
        self.machines[job * self.num_machines + op]
    }
    pub fn op_with_machine(&self, job: usize, machine: usize) -> usize {
        for i in 0..self.num_machines {
            if self.machine(job, i) == machine {
                return i;
            }
        }
        panic!("This job is missing a machine")
    }

    /// Computes a lower bound on the makespan as the maximum of the operation durations in each
    /// job and on each machine.
    pub fn makespan_lower_bound(&self) -> i32 {
        let max_by_jobs: i32 = (0..self.num_jobs)
            .map(|job| (0..self.num_machines).map(|task| self.duration(job, task)).sum::<i32>())
            .max()
            .unwrap();

        let max_by_machine: i32 = (1..self.num_machines + 1)
            .map(|m| {
                (0..self.num_jobs)
                    .map(|job| self.duration(job, self.op_with_machine(job, m)))
                    .sum()
            })
            .max()
            .unwrap();

        max_by_jobs.max(max_by_machine)
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, PartialEq, Eq, Hash)]
struct TVar(usize);

impl From<TVar> for usize {
    fn from(t: TVar) -> Self {
        t.0
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "jobshop")]
struct Opt {
    /// File containing the jobshop instance to solve.
    file: String,
    /// Output file to write the solution
    #[structopt(long = "output", short = "o")]
    output: Option<String>,
    /// When set, the solver will fail if the found solution does not have this makespan.
    #[structopt(long = "expected-makespan")]
    expected_makespan: Option<u32>,
    #[structopt(long = "lower-bound", default_value = "0")]
    lower_bound: u32,
    #[structopt(long = "upper-bound", default_value = "100000")]
    upper_bound: u32,
    /// Search strategy to use: [activity, est, parallel]
    #[structopt(long = "search", default_value = "parallel")]
    search: SearchStrategy,
}

/// Search strategies that can be added to the solver.
#[derive(Eq, PartialEq, Debug)]
enum SearchStrategy {
    /// Activity based search
    Activity,
    /// Variable selection based on earliest starting time + least slack
    Est,
    /// Run both Activity and Est in parallel.
    Parallel,
}
impl FromStr for SearchStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "act" | "activity" => Ok(SearchStrategy::Activity),
            "est" => Ok(SearchStrategy::Est),
            "par" | "parallel" => Ok(SearchStrategy::Parallel),
            e => Err(format!("Unrecognized option: '{}'", e)),
        }
    }
}

fn main() {
    let start_time = std::time::Instant::now();
    let opt = Opt::from_args();
    let filecontent = fs::read_to_string(opt.file).expect("Cannot read file");

    let pb = parse(&filecontent);

    println!("{:?}", pb);

    let lower_bound = (opt.lower_bound).max(pb.makespan_lower_bound() as u32);
    println!("Initial lower bound: {}", lower_bound);

    let (model, makespan, var_map) = encode(&pb, lower_bound, opt.upper_bound);

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));

    let est_brancher = EstBrancher {
        pb: pb.clone(),
        var_map: var_map.clone(),
        saved: DecLvl::ROOT,
    };
    let mut solver = get_solver(solver, opt.search, est_brancher);

    let result = solver.minimize(makespan).unwrap();

    if let Some((optimum, solution)) = result {
        println!("Found optimal solution with makespan: {}", optimum);
        assert_eq!(solution.var_domain(makespan).lb, optimum);

        // Format the solution in resource order : each machine is given an ordered list of tasks to process.
        let mut formatted_solution = String::new();
        for m in pb.machines() {
            // all tasks on this machine
            let mut tasks = Vec::new();
            for j in 0..pb.num_jobs {
                let op = pb.op_with_machine(j, m);
                let task = pb.tvar(j, op);
                let start_var = var_map[&task];
                let start_time = solution.var_domain(start_var).lb;
                tasks.push(((j, op), start_time));
            }
            // sort task by their start time
            tasks.sort_by_key(|(_task, start_time)| *start_time);
            write!(formatted_solution, "Machine {}:\t", m).unwrap();
            for ((job, op), _) in tasks {
                write!(formatted_solution, "({}, {})\t", job, op).unwrap();
            }
            writeln!(formatted_solution).unwrap();
        }
        println!("\n=== Solution (resource order) ===");
        print!("{}", formatted_solution);
        println!("=================================\n");

        if let Some(output) = &opt.output {
            // write solution to file
            std::fs::write(output, formatted_solution).unwrap();
        }

        solver.print_stats();
        if let Some(expected) = opt.expected_makespan {
            assert_eq!(
                optimum as u32, expected,
                "The makespan found ({}) is not the expected one ({})",
                optimum, expected
            );
        }
    } else {
        eprintln!("NO SOLUTION");
        assert!(opt.expected_makespan.is_none(), "Expected a valid solution");
    }
    println!("TOTAL RUNTIME: {:.6}", start_time.elapsed().as_secs_f64());
}

fn parse(input: &str) -> JobShop {
    let mut lines = input.lines();
    lines.next(); // drop header "num_jobs num_machines"
    let x: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
    let num_jobs = x[0].parse().unwrap();
    let num_machines = x[1].parse().unwrap();

    lines.next(); // drop "Times" line
    let mut times = Vec::with_capacity(num_machines * num_jobs);
    for _ in 0..num_jobs {
        for t in lines.next().unwrap().split_whitespace() {
            times.push(t.parse().unwrap())
        }
    }
    lines.next(); // drop "Machines" line
    let mut machines = Vec::with_capacity(num_machines * num_jobs);
    for _ in 0..num_jobs {
        for t in lines.next().unwrap().split_whitespace() {
            machines.push(t.parse().unwrap())
        }
    }

    JobShop {
        num_jobs,
        num_machines,
        times,
        machines,
    }
}

fn encode(pb: &JobShop, lower_bound: u32, upper_bound: u32) -> (Model, IVar, HashMap<TVar, IVar>) {
    let lower_bound = lower_bound as i32;
    let upper_bound = upper_bound as i32;
    let mut m = Model::new();
    let mut hmap: HashMap<TVar, IVar> = HashMap::new();

    let makespan_variable = m.new_ivar(lower_bound, upper_bound, Var::Makespan);
    for j in 0..pb.num_jobs {
        for i in 0..pb.num_machines {
            let tji = pb.tvar(j, i);
            let task_start = m.new_ivar(0, upper_bound, Var::Start(j, i));
            hmap.insert(tji, task_start);

            let left_on_job: i32 = (i..pb.num_machines).map(|t| pb.duration(j, t)).sum();
            m.enforce(leq(task_start + left_on_job, makespan_variable));

            if i > 0 {
                let end_of_previous = hmap[&pb.tvar(j, i - 1)] + pb.duration(j, i - 1);
                m.enforce(leq(end_of_previous, task_start));
            }
        }
    }
    for machine in 1..(pb.num_machines + 1) {
        for j1 in 0..pb.num_jobs {
            for j2 in (j1 + 1)..pb.num_jobs {
                let i1 = pb.op_with_machine(j1, machine);
                let i2 = pb.op_with_machine(j2, machine);

                let tji1 = hmap[&pb.tvar(j1, i1)];
                let tji2 = hmap[&pb.tvar(j2, i2)];
                let o1 = m.reify(leq(tji1 + pb.duration(j1, i1), tji2));
                let o2 = m.reify(leq(tji2 + pb.duration(j2, i2), tji1));
                m.enforce(or([o1, o2]));
            }
        }
    }
    (m, makespan_variable, hmap)
}

struct ResourceOrderingFirst;
impl Heuristic<Var> for ResourceOrderingFirst {
    fn decision_stage(&self, _var: VarRef, label: Option<&Var>, _model: &aries_model::Model<Var>) -> u8 {
        match label {
            Some(&Var::Reified) => 0, // branch first on reifications of the ordering constraints
            _ => 1,
        }
    }
}

/// Builds a solver for the given strategy.
fn get_solver(base: Solver, strategy: SearchStrategy, est_brancher: EstBrancher) -> ParSolver {
    let base_solver = Box::new(base);
    let make_act = |s: &mut Solver| s.set_brancher(ActivityBrancher::new_with_heuristic(ResourceOrderingFirst));
    let make_est = |s: &mut Solver| s.set_brancher(est_brancher.clone());
    match strategy {
        SearchStrategy::Activity => ParSolver::new(base_solver, 1, |_, s| make_act(s)),
        SearchStrategy::Est => ParSolver::new(base_solver, 1, |_, s| make_est(s)),
        SearchStrategy::Parallel => ParSolver::new(base_solver, 2, |id, s| match id {
            0 => make_act(s),
            1 => make_est(s),
            _ => unreachable!(),
        }),
    }
}

#[derive(Clone)]
struct EstBrancher {
    pb: JobShop,
    var_map: HashMap<TVar, IVar>,
    saved: DecLvl,
}

impl SearchControl<Var> for EstBrancher {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        let active_in_job = |j: usize| {
            for t in 0..self.pb.num_machines {
                let v = self.var_map[&self.pb.tvar(j, t)];
                let (lb, ub) = model.domain_of(v);
                if lb < ub {
                    return Some((v, lb, ub));
                }
            }
            None
        };
        // for each job selects the first task whose start time is not fixed yet
        let active_tasks = self.pb.jobs().filter_map(active_in_job);
        // among the task with the smallest "earliest starting time (est)" pick the one that has the least slack
        let best = active_tasks.min_by_key(|(_var, est, lst)| (*est, *lst));

        // decision is to set the start time to the selected task to the smallest possible value.
        // if no task was selected, it means that they are all instantiated and we have a complete schedule
        best.map(|(var, est, _)| Decision::SetLiteral(Lit::leq(var, est)))
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for EstBrancher {
    fn save_state(&mut self) -> DecLvl {
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.saved -= 1;
    }
}
