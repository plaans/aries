mod parser;
mod problem;
mod search;

use crate::problem::ProblemKind;
use crate::search::{SearchStrategy, Solver, Var};
use anyhow::*;
use aries_model::extensions::{AssignmentExt, Shaped};
use aries_model::lang::IVar;
use aries_stn::theory::{StnConfig, StnTheory};
use std::fmt::Write;
use std::fs;
use structopt::StructOpt;
use walkdir::WalkDir;

#[derive(Debug, StructOpt)]
#[structopt(name = "aries-scheduler")]
pub struct Opt {
    /// Kind of the problem to be solved in {jobshop, openshop}
    kind: ProblemKind,
    /// File containing the instance to solve.
    file: String,
    /// Output file to write the solution
    #[structopt(long = "output", short = "o")]
    output: Option<String>,
    /// When set, the solver will fail with an exit code of 1 if the found solution does not have this makespan.
    #[structopt(long = "expected-makespan")]
    expected_makespan: Option<u32>,
    #[structopt(long = "lower-bound", default_value = "0")]
    lower_bound: u32,
    #[structopt(long = "upper-bound", default_value = "100000")]
    upper_bound: u32,
    /// Search strategy to use in {activity, est, parallel}
    #[structopt(long = "search", default_value = "learning-rate")]
    search: SearchStrategy,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    let file = &opt.file;
    if std::fs::metadata(file)?.is_file() {
        solve(opt.kind, &opt.file, &opt);
        Ok(())
    } else {
        for entry in WalkDir::new(file).follow_links(true).into_iter().filter_map(|e| e.ok()) {
            let f_name = entry.file_name().to_string_lossy();
            if f_name.ends_with(".txt") {
                println!("{f_name}");
                solve(opt.kind, &entry.path().to_string_lossy(), &opt);
            }
        }
        Ok(())
    }
}

fn solve(kind: ProblemKind, instance: &str, opt: &Opt) {
    let start_time = std::time::Instant::now();
    let filecontent = fs::read_to_string(instance).expect("Cannot read file");
    let pb = match kind {
        ProblemKind::OpenShop => parser::openshop(&filecontent),
        ProblemKind::JobShop => parser::jobshop(&filecontent),
    };
    assert_eq!(pb.kind, kind);
    // println!("{:?}", pb);

    let lower_bound = (opt.lower_bound).max(pb.makespan_lower_bound() as u32);
    println!("Initial lower bound: {lower_bound}");

    let model = problem::encode(&pb, lower_bound, opt.upper_bound);
    let makespan: IVar = IVar::new(model.shape.get_variable(&Var::Makespan).unwrap());

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));

    let mut solver = search::get_solver(solver, opt.search, &pb);

    let result = solver.minimize(makespan).unwrap();

    if let Some((optimum, solution)) = result {
        println!("Found optimal solution with makespan: {optimum}");
        assert_eq!(solution.var_domain(makespan).lb, optimum);

        // Format the solution in resource order : each machine is given an ordered list of tasks to process.
        let mut formatted_solution = String::new();
        for m in pb.machines() {
            // all tasks on this machine
            let mut tasks = Vec::new();
            for j in 0..pb.num_jobs {
                let task = Var::Start(j, m);
                let start_var = solver.get_int_var(&task).unwrap();
                let start_time = solution.var_domain(start_var).lb;
                tasks.push(((j, m), start_time));
            }
            // sort task by their start time
            tasks.sort_by_key(|(_task, start_time)| *start_time);
            write!(formatted_solution, "Machine {m}:\t").unwrap();
            for ((job, op), _) in tasks {
                write!(formatted_solution, "({job}, {op})\t").unwrap();
            }
            writeln!(formatted_solution).unwrap();
        }
        // println!("\n=== Solution (resource order) ===");
        // print!("{}", formatted_solution);
        // println!("=================================\n");

        if let Some(output) = &opt.output {
            // write solution to file
            std::fs::write(output, formatted_solution).unwrap();
        }

        solver.print_stats();
        if let Some(expected) = opt.expected_makespan {
            assert_eq!(
                optimum as u32, expected,
                "The makespan found ({optimum}) is not the expected one ({expected})"
            );
        }
        println!("XX\t{}\t{}\t{}", instance, optimum, start_time.elapsed().as_secs_f64());
    } else {
        solver.print_stats();
        eprintln!("NO SOLUTION");
        assert!(opt.expected_makespan.is_none(), "Expected a valid solution");
    }
    println!("TOTAL RUNTIME: {:.6}", start_time.elapsed().as_secs_f64());
}
