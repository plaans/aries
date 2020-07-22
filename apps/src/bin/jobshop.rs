#![allow(dead_code)]

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
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, PartialEq, Eq, Hash)]
struct TVar(usize);

impl Into<usize> for TVar {
    fn into(self) -> usize {
        self.0
    }
}

use aries_collections::MinVal;
use aries_sat::all::BVar;
use aries_sat::SearchParams;
use aries_smt::{SMTSolver, Theory};
use aries_tnet::stn::Edge as STNEdge;
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
}

fn main() {
    let opt = Opt::from_args();
    let filecontent = fs::read_to_string(opt.file).expect("Cannot read file");

    let pb = parse(&filecontent);

    println!("{:?}", pb);

    let (mut smt, makespan_var) = init_jobshop_solver(&pb, opt.upper_bound);
    let x = smt.theory.propagate_all();
    assert_eq!(x, NetworkStatus::Consistent);

    // find initial solution
    smt.theory.set_backtrack_point();
    smt.solve();
    let mut makespan = smt.theory.lb(makespan_var);
    println!("makespan: {}", makespan);

    let use_lns = opt.lns.unwrap_or(true);

    let optimal_makespan = loop {
        smt.theory.backtrack();
        smt.theory.add_edge(smt.theory.origin(), makespan_var, makespan - 1);
        match smt.theory.propagate_all() {
            NetworkStatus::Consistent => (),
            NetworkStatus::Inconsistent(_) => {
                break makespan;
            }
        }
        smt.theory.set_backtrack_point();
        match smt.solve() {
            Some(_model) => {
                makespan = smt.theory.lb(makespan_var);
                println!("Improved makespan: {}", makespan);
                if use_lns {
                    // Mimic Large-Neighborhood Search (LNS) behavior :
                    // The polarity (i.e. preferred value) of each variable to the value
                    // it takes in the best solution.
                    // This will make the planner explore variations of the current solution in an
                    // attempt to improve it.
                    for var in smt.sat.variables() {
                        match smt.sat.get_variable(var) {
                            Some(x) => smt.sat.set_polarity(var, x),
                            None => unreachable!("All variables should have been set."),
                        }
                    }
                }
                assert!(makespan >= opt.lower_bound as i32);
                if makespan == opt.lower_bound as i32 {
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

fn init_jobshop_solver(pb: &JobShop, upper_bound: u32) -> (SMTSolver<STNEdge<i32>, IncSTN<i32>>, u32) {
    let mut hmap = HashMap::new();
    let mut stn = IncSTN::new();
    let makespan = stn.add_node(0, upper_bound as i32);
    for j in 0..pb.num_jobs {
        for i in 0..pb.num_machines {
            let tji = pb.tvar(j, i);
            let x = stn.add_node(0, upper_bound as i32);
            hmap.insert(tji, x);
            let left_on_job: i32 = (i..pb.num_machines).map(|t| pb.duration(j, t)).sum();
            stn.add_edge(makespan, x, -left_on_job);
            if i > 0 {
                stn.add_edge(x, hmap[&pb.tvar(j, i - 1)], -pb.duration(j, i - 1));
            }
        }
    }
    let mut mapping = aries_smt::Mapping::default();
    let mut next_var = BVar::min_value();
    let mut num_vars: usize = 0;

    for m in 1..(pb.num_machines + 1) {
        for j1 in 0..pb.num_jobs {
            for j2 in (j1 + 1)..pb.num_jobs {
                let i1 = pb.op_with_machine(j1, m);
                let i2 = pb.op_with_machine(j2, m);
                let v = next_var;
                next_var = next_var.next();
                num_vars += 1;

                let tji1 = hmap[&pb.tvar(j1, i1)];
                let tji2 = hmap[&pb.tvar(j2, i2)];
                let c1 = stn.add_inactive_edge(tji2, tji1, -pb.duration(j1, i1));
                let c2 = stn.add_inactive_edge(tji1, tji2, -pb.duration(j2, i2));
                mapping.bind(v.true_lit(), c1 as u32);
                mapping.bind(v.false_lit(), c2 as u32);
                println!("recorded constraint : ({},{}) != ({},{}) [ v : {}] ", j1, i1, j2, i1, v)
            }
        }
    }
    let sat = aries_sat::Solver::new(num_vars as u32, SearchParams::default());

    (SMTSolver::new(sat, stn, mapping), makespan)
}
