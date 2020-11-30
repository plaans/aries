#![allow(dead_code)]

use aries_smt::backtrack::Backtrack;

#[derive(Debug)]
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

impl Into<usize> for TVar {
    fn into(self) -> usize {
        self.0
    }
}

use aries_sat::all::DecisionLevel;
use aries_sat::SatProblem;
use aries_smt::lang::{BAtom, IAtom, IVar, Interner};
use aries_smt::modules::ModularSMT;
use aries_smt::solver::SMTSolver;
use aries_smt::Embeddable;
use aries_tnet::min_delay;
use aries_tnet::stn::{DiffLogicTheory, Edge as STNEdge, Timepoint};
use aries_tnet::stn::{IncSTN, NetworkStatus};
use std::collections::HashMap;
use std::fs;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "jobshop")]
struct Opt {
    file: String,
    #[structopt(long = "expected-makespan")]
    expected_makespan: Option<u32>,
    #[structopt(long = "lower-bound", default_value = "0")]
    lower_bound: u32,
    #[structopt(long = "upper-bound", default_value = "100000")]
    upper_bound: u32,
    /// Mimics Large Neighborhood Search (LNS) like behavior by setting the preferred value of
    /// variables to their value in the best solution.
    #[structopt(long = "lns")]
    lns: Option<bool>,
    /// If set to true, the solver will use a lazy SMT approach.
    /// If set to false, the solver will use an eager SMT approach.
    /// If unset, the solver is free to use its preferred approach
    #[structopt(long)]
    lazy: Option<bool>,
}

fn main() {
    let opt = Opt::from_args();
    let filecontent = fs::read_to_string(opt.file).expect("Cannot read file");

    let pb = parse(&filecontent);

    println!("{:?}", pb);

    let use_lns = opt.lns.unwrap_or(true);
    let use_lazy = opt.lazy.unwrap_or(true);
    {
        let (model, constraints, makespan) = encode(&pb, opt.upper_bound);
        let mut solver = ModularSMT::new(model);
        solver.add_theory(Box::new(DiffLogicTheory::new()));
        solver.enforce(&constraints);
        solver.solve();

        let mut optimal = None;

        while let Some((lb, _)) = solver.domain_of(makespan) {
            println!("SAT: Makespan: {}", lb);
            optimal = Some(lb);
            solver.reset();
            let improved = solver.interner.lt(makespan, lb);
            solver.enforce(&[improved]);
            println!("Adding constraint: makespan < {}", lb);

            if !solver.solve() {
                println!("unsat");
                break;
            }
        }
        match optimal {
            Some(makespan) => println!("Found optimal solution with makespan: {}", makespan),
            None => println!("Invalid problem"),
        }
    }
    return;
    let (mut smt, makespan_var) = init_jobshop_solver(&pb, opt.upper_bound);
    let x = smt.theory.propagate_all();
    assert_eq!(x, NetworkStatus::Consistent);
    let lower_bound = (opt.lower_bound as i32)
        .max(smt.theory.lb(makespan_var))
        .max(pb.makespan_lower_bound());
    println!("Initial lower bound: {}", lower_bound);

    // find initial solution
    let mut lvl = smt.theory.set_backtrack_point();
    smt.solve(use_lazy);
    let mut makespan = smt.theory.lb(makespan_var);
    println!("Found initial solution.\nMakespan: {}", makespan);

    let optimal_makespan = loop {
        smt.theory.backtrack_to(lvl);
        // TODO: allow the addition of persistent constraints to a theory to avoid the need to backtrack
        //       all the way to the ground
        smt.sat.backtrack_to(DecisionLevel::GROUND);
        smt.theory.add_edge(smt.theory.origin(), makespan_var, makespan - 1);
        match smt.theory.propagate_all() {
            NetworkStatus::Consistent => (),
            NetworkStatus::Inconsistent(_) => {
                break makespan;
            }
        }
        lvl = smt.theory.set_backtrack_point();
        match smt.solve(use_lazy) {
            Some(_model) => {
                makespan = smt.theory.lb(makespan_var);
                println!("Improved makespan: {}", makespan);
                if use_lns {
                    // Mimic Large-Neighborhood Search (LNS) behavior :
                    // The polarity (i.e. preferred value) of each variable is set to the value
                    // it takes in the best solution.
                    // This will make the solver explore variations of the current solution in an
                    // attempt to improve it.
                    for var in smt.sat.variables() {
                        match smt.sat.get_variable(var) {
                            Some(x) => smt.sat.set_polarity(var, x),
                            None => unreachable!("All variables should have been set."),
                        }
                    }
                }
                assert!(makespan >= lower_bound);
                if makespan == lower_bound {
                    break makespan;
                }
            }
            None => {
                break makespan;
            }
        }
    };
    println!("Optimal solution found: {}", optimal_makespan);
    println!("{}", smt.sat.stats);
    if let Some(target) = opt.expected_makespan {
        if optimal_makespan != target as i32 {
            eprintln!("Error: expected an optimal makespan of {}", target);
            std::process::exit(1);
        }
    }
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

type Solver = SMTSolver<STNEdge<i32>, IncSTN<i32>>;

fn encode(pb: &JobShop, upper_bound: u32) -> (Interner, Vec<BAtom>, IVar) {
    let upper_bound = upper_bound as i32;
    let mut l = Interner::default();
    let mut hmap: HashMap<TVar, IVar> = HashMap::new();
    let mut constraints = Vec::new();

    let makespan_variable = l.new_ivar(0, upper_bound, "makespan");
    for j in 0..pb.num_jobs {
        for i in 0..pb.num_machines {
            let tji = pb.tvar(j, i);
            let task_start = l.new_ivar(0, upper_bound, format!("start({}, {})", j, i));
            hmap.insert(tji, task_start);

            let left_on_job: i32 = (i..pb.num_machines).map(|t| pb.duration(j, t)).sum();
            constraints.push(l.leq(task_start + left_on_job, makespan_variable));

            if i > 0 {
                let end_of_previous = hmap[&pb.tvar(j, i - 1)] + pb.duration(j, i - 1);
                constraints.push(l.leq(end_of_previous, task_start));
            }
        }
    }
    for m in 1..(pb.num_machines + 1) {
        for j1 in 0..pb.num_jobs {
            for j2 in (j1 + 1)..pb.num_jobs {
                let i1 = pb.op_with_machine(j1, m);
                let i2 = pb.op_with_machine(j2, m);

                let tji1 = hmap[&pb.tvar(j1, i1)];
                let tji2 = hmap[&pb.tvar(j2, i2)];
                let o1 = l.leq(tji1 + pb.duration(j1, i1), tji2);
                let o2 = l.leq(tji2 + pb.duration(j2, i2), tji1);
                constraints.push(l.or2(o1, o2));
            }
        }
    }

    (l, constraints, makespan_variable)
}

fn init_jobshop_solver(pb: &JobShop, upper_bound: u32) -> (Solver, Timepoint) {
    let mut solver: Solver = SMTSolver::default();
    let mut hmap: HashMap<TVar, Timepoint> = HashMap::new();

    let makespan_variable: Timepoint = solver.theory.add_timepoint(0, upper_bound as i32);
    for j in 0..pb.num_jobs {
        for i in 0..pb.num_machines {
            let tji = pb.tvar(j, i);
            let x = solver.theory.add_timepoint(0, upper_bound as i32);
            hmap.insert(tji, x);
            let left_on_job: i32 = (i..pb.num_machines).map(|t| pb.duration(j, t)).sum();
            let job_ends_before_makespan = min_delay(x, makespan_variable, left_on_job).embed(&mut solver);
            solver.enforce(job_ends_before_makespan);
            if i > 0 {
                let starts_after_previous =
                    min_delay(hmap[&pb.tvar(j, i - 1)], x, pb.duration(j, i - 1)).embed(&mut solver);
                solver.enforce(starts_after_previous);
            }
        }
    }

    for m in 1..(pb.num_machines + 1) {
        for j1 in 0..pb.num_jobs {
            for j2 in (j1 + 1)..pb.num_jobs {
                let i1 = pb.op_with_machine(j1, m);
                let i2 = pb.op_with_machine(j2, m);

                let tji1 = hmap[&pb.tvar(j1, i1)];
                let tji2 = hmap[&pb.tvar(j2, i2)];
                let non_overlapping = [
                    min_delay(tji1, tji2, pb.duration(j1, i1)).embed(&mut solver),
                    min_delay(tji2, tji1, pb.duration(j2, i2)).embed(&mut solver),
                ];
                solver.add_clause(&non_overlapping);
                println!("recorded constraint : ({},{}) != ({},{})  ", j1, i1, j2, i1);
            }
        }
    }

    (solver, makespan_variable)
}
