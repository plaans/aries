#![allow(dead_code)]

use aries_smt::model::assignments::Assignment;

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

use aries_smt::model::lang::{BAtom, IVar};
use aries_smt::solver::SMTSolver;

use aries_smt::model::Model;
use aries_tnet::stn::DiffLogicTheory;
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

    let _use_lns = opt.lns.unwrap_or(true);
    let _use_lazy = opt.lazy.unwrap_or(true);
    let start = std::time::Instant::now();
    let (model, constraints, makespan) = encode(&pb, opt.upper_bound);
    let mut solver = SMTSolver::new(model);
    solver.add_theory(Box::new(DiffLogicTheory::new()));
    solver.enforce(&constraints);

    let result = solver.minimize_with(makespan, |objective, _| {
        println!("New solution with makespan: {}", objective)
    });

    if let Some((optimum, solution)) = result {
        println!("Found optimal solution with makespan: {}", optimum);
        assert_eq!(solution.lower_bound(makespan), optimum);
        for v in solver.model.ints.variables() {
            println!("{} <- {}", solver.model.fmt(v), solution.lower_bound(v));
        }
    } else {
        eprintln!("NO SOLUTION");
    }
    println!("RUNTIME: {:.6}", start.elapsed().as_secs_f64());

    // let (mut smt, makespan_var) = init_jobshop_solver(&pb, opt.upper_bound);
    // let x = smt.theory.propagate_all();
    // assert_eq!(x, NetworkStatus::Consistent);
    // let lower_bound = (opt.lower_bound as i32)
    //     .max(smt.theory.lb(makespan_var))
    //     .max(pb.makespan_lower_bound());
    // println!("Initial lower bound: {}", lower_bound);
    //
    // // find initial solution
    // let mut lvl = smt.theory.set_backtrack_point();
    // smt.solve(use_lazy);
    // let mut makespan = smt.theory.lb(makespan_var);
    // println!("Found initial solution.\nMakespan: {}", makespan);
    //
    // let optimal_makespan = loop {
    //     smt.theory.backtrack_to(lvl);
    //     // TODO: allow the addition of persistent constraints to a theory to avoid the need to backtrack
    //     //       all the way to the ground
    //     smt.sat.backtrack_to(DecisionLevel::GROUND);
    //     smt.theory.add_edge(smt.theory.origin(), makespan_var, makespan - 1);
    //     match smt.theory.propagate_all() {
    //         NetworkStatus::Consistent => (),
    //         NetworkStatus::Inconsistent(_) => {
    //             break makespan;
    //         }
    //     }
    //     lvl = smt.theory.set_backtrack_point();
    //     match smt.solve(use_lazy) {
    //         Some(_model) => {
    //             makespan = smt.theory.lb(makespan_var);
    //             println!("Improved makespan: {}", makespan);
    //             if use_lns {
    //                 // Mimic Large-Neighborhood Search (LNS) behavior :
    //                 // The polarity (i.e. preferred value) of each variable is set to the value
    //                 // it takes in the best solution.
    //                 // This will make the solver explore variations of the current solution in an
    //                 // attempt to improve it.
    //                 for var in smt.sat.variables() {
    //                     match smt.sat.get_variable(var) {
    //                         Some(x) => smt.sat.set_polarity(var, x),
    //                         None => unreachable!("All variables should have been set."),
    //                     }
    //                 }
    //             }
    //             assert!(makespan >= lower_bound);
    //             if makespan == lower_bound {
    //                 break makespan;
    //             }
    //         }
    //         None => {
    //             break makespan;
    //         }
    //     }
    // };
    // println!("Optimal solution found: {}", optimal_makespan);
    // println!("{}", smt.sat.stats);
    // if let Some(target) = opt.expected_makespan {
    //     if optimal_makespan != target as i32 {
    //         eprintln!("Error: expected an optimal makespan of {}", target);
    //         std::process::exit(1);
    //     }
    // }
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

fn encode(pb: &JobShop, upper_bound: u32) -> (Model, Vec<BAtom>, IVar) {
    let upper_bound = upper_bound as i32;
    let mut m = Model::default();
    let mut hmap: HashMap<TVar, IVar> = HashMap::new();
    let mut constraints = Vec::new();

    let makespan_variable = m.new_ivar(0, upper_bound, "makespan");
    for j in 0..pb.num_jobs {
        for i in 0..pb.num_machines {
            let tji = pb.tvar(j, i);
            let task_start = m.new_ivar(0, upper_bound, format!("start({}, {})", j, i));
            hmap.insert(tji, task_start);

            let left_on_job: i32 = (i..pb.num_machines).map(|t| pb.duration(j, t)).sum();
            constraints.push(m.leq(task_start + left_on_job, makespan_variable));

            if i > 0 {
                let end_of_previous = hmap[&pb.tvar(j, i - 1)] + pb.duration(j, i - 1);
                constraints.push(m.leq(end_of_previous, task_start));
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
                let o1 = m.leq(tji1 + pb.duration(j1, i1), tji2);
                let o2 = m.leq(tji2 + pb.duration(j2, i2), tji1);
                constraints.push(m.or2(o1, o2));
            }
        }
    }

    (m, constraints, makespan_variable)
}
