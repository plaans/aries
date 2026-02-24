mod parser;
mod problem;
mod search;

use crate::problem::{Encoding, OperationId, Problem, ProblemKind};
use crate::search::{SearchStrategy, Solver, Var};
use anyhow::Context;
use aries::model::lang::IVar;
use aries::prelude::*;
use aries::solver::{Exit, SearchLimit};
use aries_bench::IntermediateResult;
use std::fmt::Write;
use std::path::Path;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use walkdir::WalkDir;

#[derive(Debug, StructOpt)]
#[structopt(name = "aries-scheduler")]
pub struct Opt {
    /// Kind of the problem to be solved in {jobshop, openshop, flexible}
    kind: ProblemKind,
    /// File containing the instance to solve.
    files: Vec<String>,
    /// Output file to write the solution
    #[structopt(long = "output", short = "o")]
    output: Option<String>,
    /// When set, the solver will fail with an exit code of 1 if the found solution does not have this makespan.
    #[structopt(long = "expected-makespan")]
    expected_makespan: Option<u32>,
    #[structopt(long = "lower-bound")]
    lower_bound: Option<u32>,
    #[structopt(long = "upper-bound")]
    upper_bound: Option<u32>,
    /// Search strategy to use
    #[structopt(long = "search", default_value = "default")]
    search: SearchStrategy,
    /// maximum runtime, in seconds.
    #[structopt(long = "timeout", short = "t")]
    timeout: Option<u32>,
    /// If set, a summary of the run will be saved in the indicated directory.
    /// This option is intended to ease the collection of benchmark results with `aries-bench`
    #[structopt(long = "report", short = "r")]
    report: Option<String>,
    /// Choose the propagation level for the no-overlap constraint.
    /// Options: try it out, you will get an error message with the options
    #[structopt(long = "no-overlap", default_value = "edge-finding")]
    no_overlap: aries::reasoners::cp::disjunctive::PropagatorKind,
    /// Indicates a layout file, containing a matrix with the transportation times between all pairs of machines.
    #[structopt(long = "layout")]
    layout_file: Option<String>,
    /// Indicates a maximum delay (time lag) between two operations of the same job
    #[structopt(long = "time-lag")]
    time_lag: Option<u32>,
}

fn main() -> anyhow::Result<()> {
    // Terminate the process if a thread panics.
    // take_hook() returns the default hook in case when a custom one is not set
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        std::process::exit(1);
    }));
    // read command line arguments
    let opt = Opt::from_args();

    for file in &opt.files {
        if std::fs::metadata(file)?.is_file() {
            solve(opt.kind, file, &opt)?;
        } else {
            for entry in WalkDir::new(file).follow_links(true).into_iter().filter_map(|e| e.ok()) {
                let f_name = entry.file_name().to_string_lossy();
                if f_name.ends_with(".txt") {
                    println!("{f_name}");
                    solve(opt.kind, &entry.path().to_string_lossy(), &opt)?;
                }
            }
        }
    }
    Ok(())
}

fn solve(kind: ProblemKind, instance: &str, opt: &Opt) -> anyhow::Result<()> {
    let deadline = opt
        .timeout
        .map(|dur| SearchLimit::Deadline(Instant::now() + Duration::from_secs(dur as u64)))
        .unwrap_or(SearchLimit::None);
    let start_time = std::time::Instant::now();
    let filecontent = read_file(instance)?;
    let mut pb = match kind {
        ProblemKind::OpenShop => parser::openshop(&filecontent),
        ProblemKind::JobShop => parser::jobshop(&filecontent),
        ProblemKind::FlexibleShop => parser::flexshop(&filecontent),
    };
    if let Some(layout) = opt.layout_file.as_ref() {
        let file_content = read_file(layout)?;
        let transport_times = parser::transport_time(&file_content);
        pb.set_transport_times(transport_times);
    }
    if let Some(time_lag) = opt.time_lag {
        pb.set_time_lag(time_lag);
    }
    assert_eq!(pb.kind, kind);
    // println!("{:?}", pb);

    let lower_bound = (opt.lower_bound.unwrap_or(0)).max(pb.makespan_lower_bound() as u32);
    println!("Initial lower bound: {lower_bound}");

    let (model, encoding) = problem::encode(&pb, lower_bound, opt.upper_bound, opt.no_overlap);
    let makespan: IVar = IVar::new(model.shape.get_variable(&Var::Makespan).unwrap());

    let solver = Solver::new(model);
    let mut solver = search::get_solver(solver, &opt.search, &encoding);
    let mut solution_history: Vec<IntermediateResult> = Default::default();

    let mut best = Option::None;
    let result = solver.minimize_with_callback(
        makespan,
        |obj, sol| {
            println!("New solution with makespan: {}", obj);
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
            let optimum = solution.value_of(makespan).unwrap();
            assert_eq!(obj, optimum); // sanity check
            println!("> OPTIMAL (cost: {optimum})");

            if let Some(expected) = opt.expected_makespan {
                assert_eq!(
                    optimum as u32, expected,
                    "The makespan found ({optimum}) is not the expected one ({expected})"
                );
            }
            println!("XX\t{}\t{}\t{}", instance, optimum, start_time.elapsed().as_secs_f64());
            aries_bench::SolveStatus::Solved
        }
        Ok(None) => {
            println!("> UNSATISFIABLE");
            assert!(opt.expected_makespan.is_none(), "Expected a valid solution");
            aries_bench::SolveStatus::Solved
        }
        Err(Exit::Interrupted) => match best.as_ref() {
            Some(sol) => {
                let best_cost = sol.value_of(makespan).unwrap();
                println!("> TIMEOUT (best solution cost {best_cost})");
                aries_bench::SolveStatus::Timeout
            }
            None => {
                println!("> TIMEOUT (no solution found)");
                aries_bench::SolveStatus::Timeout
            }
        },
    };
    if let Some(solution) = best.as_ref() {
        // export the solution to file if specified
        export(solution, &pb, &encoding, opt.output.as_ref());
    }
    if let Some(report_dir) = opt.report.as_ref() {
        let mut problem = aries_bench::Problem {
            name: instance.to_string(),
            timeout: opt
                .timeout
                .map(|t| Duration::from_secs(t as u64))
                .unwrap_or(Duration::MAX),
            flags: Default::default(),
        };
        if let Some(lb) = opt.lower_bound {
            problem.flags.insert("lb".to_string(), lb.to_string());
        }
        if let Some(ub) = opt.upper_bound {
            problem.flags.insert("ub".to_string(), ub.to_string());
        }
        if let Some(layout) = opt.layout_file.as_ref() {
            problem.flags.insert("layout".to_string(), layout.to_string());
        }
        if let Some(time_lag) = opt.time_lag {
            problem.flags.insert("time-lag".to_string(), time_lag.to_string());
        }

        let result = aries_bench::SolveResult {
            problem,
            status,
            runtime: start_time.elapsed(),
            objective_value: best.map(|sol| sol.value_of(makespan).unwrap() as i64),
            metrics: Default::default(),
            objective_history: solution_history,
        }
        .with_metric(aries_bench::Metric::NumConflicts, solver.stats.num_conflicts as f64)
        .with_metric(aries_bench::Metric::NumDecisions, solver.stats.num_decisions as f64)
        .with_metric(aries_bench::Metric::NumDomUpdates, solver.stats.num_dom_updates as f64);

        result.save_to_dir(report_dir)?;
    }

    println!("TOTAL RUNTIME: {:.6}", start_time.elapsed().as_secs_f64());
    Ok(())
}

/// Write the solution to file if the file is not None
fn export(solution: &Solution, pb: &Problem, encoding: &Encoding, file: Option<&String>) {
    if let Some(output_file) = file {
        let mut formatted_solution = String::new();
        for m in pb.machines() {
            // all tasks on this machine
            let mut tasks = Vec::new();
            for alt in encoding.alternatives_on_machine(m) {
                if solution.entails(alt.presence) {
                    let start_time = solution.var_domain(alt.start).lb;
                    tasks.push((alt.id, start_time));
                }
            }
            // sort task by their start time
            tasks.sort_by_key(|(_task, start_time)| *start_time);
            write!(formatted_solution, "Machine {m}:\t").unwrap();
            for (OperationId { job, op, alt }, _) in tasks {
                let alt = alt.unwrap();
                write!(formatted_solution, "({job}, {op}, {alt})\t").unwrap();
            }
            writeln!(formatted_solution).unwrap();
        }
        // println!("\n=== Solution (resource order) ===");
        // print!("{}", formatted_solution);
        // println!("=================================\n");

        // write solution to file
        std::fs::write(output_file, formatted_solution).unwrap();
    }
}

fn read_file(file: impl AsRef<Path>) -> anyhow::Result<String> {
    std::fs::read_to_string(file.as_ref()).with_context(move || format!("Cannot read file: '{:?}'", file.as_ref()))
}

#[cfg(test)]
mod test {
    use crate::problem::ProblemKind;
    use crate::search::Var;
    use crate::{parser, problem};
    use aries::core::state::witness;
    use aries::model::Label;
    use aries::prelude::*;
    use aries::reasoners::cp::disjunctive;
    use aries::solver::search::random::RandomChoice;
    use aries::solver::{SearchLimit, Solver};

    /// Solve the problem multiple with different random variable ordering, ensuring that all results are as expected.
    /// It also set up solution witness to check that no learned clause prune valid solutions.
    fn random_solves<S: Label>(model: &Model<S>, objective: IVar, num_solves: u32, expected_result: Option<IntCst>) {
        // when this object goes out of scope, any witness solution for the current thread will be removed
        let _clean_up = witness::on_drop_witness_cleaner();
        for seed in 0..num_solves {
            let model = model.clone();
            let solver = &mut Solver::new(model);
            solver.set_brancher(RandomChoice::new(seed as u64));
            let result = if let Some((makespan, assignment)) = solver
                .minimize_with_callback(
                    objective,
                    |makespan, _| {
                        if expected_result == Some(makespan) {
                            // we have found the expected solution, remove the witness because the current solution
                            // will be disallowed to force an improvement
                            witness::remove_solution_witness()
                        }
                    },
                    SearchLimit::None,
                )
                .unwrap()
            {
                println!("[{seed}] SOL: {makespan:?}");

                if expected_result == Some(makespan) {
                    // we have the expected solution, save it to be checked against
                    // when this is set, solver for the current thread will check that any learned clause does not
                    // forbid this solution
                    witness::set_solution_witness(assignment)
                }

                Some(makespan)
            } else {
                None
            };
            assert_eq!(expected_result, result);
        }
    }

    fn run_tests(kind: ProblemKind, instance: &str, opt: u32, num_reps: u32, use_constraints: bool) {
        let filecontent = std::fs::read_to_string(instance).expect("Cannot read file");
        let pb = match kind {
            ProblemKind::OpenShop => parser::openshop(&filecontent),
            ProblemKind::JobShop => parser::jobshop(&filecontent),
            ProblemKind::FlexibleShop => parser::flexshop(&filecontent),
        };
        assert_eq!(pb.kind, kind);
        let propagation_level = if use_constraints {
            disjunctive::PropagatorKind::default()
        } else {
            disjunctive::PropagatorKind::None
        };

        let lower_bound = pb.makespan_lower_bound() as u32;

        // produce a model for this problem
        let (model, _encoding) = problem::encode(&pb, lower_bound, Some(opt * 2), propagation_level);
        let makespan: IVar = IVar::new(model.shape.get_variable(&Var::Makespan).unwrap());

        // run several random solvers on the problem to assert the coherency of the results
        random_solves(&model, makespan, num_reps, Some(opt as IntCst))
    }

    #[test]
    fn test_ft06_basic() {
        run_tests(ProblemKind::JobShop, "instances/jobshop/ft06.jsp", 55, 10, false);
    }

    #[test]
    fn test_ft06_constraints() {
        run_tests(ProblemKind::JobShop, "instances/jobshop/ft06.jsp", 55, 10, true);
    }

    #[test]
    fn test_fjs_edata_mt06_basic() {
        run_tests(
            ProblemKind::FlexibleShop,
            "instances/flexible/hu/edata/mt06.fjs",
            55,
            10,
            false,
        );
    }

    #[test]
    fn test_fjs_edata_mt06_constraints() {
        run_tests(
            ProblemKind::FlexibleShop,
            "instances/flexible/hu/edata/mt06.fjs",
            55,
            10,
            true,
        );
    }

    #[test]
    fn test_fjs_rdata_mt06_basic() {
        run_tests(
            ProblemKind::FlexibleShop,
            "instances/flexible/hu/rdata/mt06.fjs",
            47,
            10,
            false,
        );
    }

    #[test]
    fn test_fjs_rdata_mt06_constraints() {
        run_tests(
            ProblemKind::FlexibleShop,
            "instances/flexible/hu/rdata/mt06.fjs",
            47,
            10,
            true,
        );
    }
}
