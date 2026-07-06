use std::path::PathBuf;
use std::time::{Duration, Instant};

use aries_bench_data::{IntermediateResult, SolveStatus, SolverMetric};
use aries_plan_engine::{generate, plans::lifted_plan::LiftedPlan};
use aries_solver::prelude::*;
use aries_solver::solver::{Exit, SearchLimit};
use clap::Parser;
use planx::{
    Res,
    pddl::{self, input::Input},
};

#[derive(Parser, Debug)]
struct Args {
    /// PDDL problem file
    problem: PathBuf,

    /// PDDL domain file (inferred if not provided)
    #[arg(short, long)]
    domain: Option<PathBuf>,

    /// Output file to write the plan
    #[arg(long)]
    output: Option<PathBuf>,

    /// Maximum runtime in seconds
    #[arg(short, long)]
    timeout: Option<u32>,

    /// Directory to save benchmark report
    #[arg(short, long)]
    report: Option<PathBuf>,

    #[command(flatten)]
    generate: generate::Options,
}

fn main() -> Res<()> {
    let args = Args::parse();
    let start_time = Instant::now();

    let (dom, pb) = parse_problem(&args)?;
    let model = pddl::build_model(&dom, &pb)?;
    println!("{model}");

    let plan = LiftedPlan::default();
    let (mut explain, encoding, _sched) =
        generate::encode_finite_planning_problem(&model, &plan, &args.generate)?;

    let objective = LinTerm::int_cst(0); // encoding.objectives.first().copied().expect("no objective");

    let deadline = args
        .timeout
        .map(|t| SearchLimit::Deadline(start_time + Duration::from_secs(t as u64)))
        .unwrap_or(SearchLimit::None);

    // Collect enabler literals before borrowing solver mutably
    let enabler_lits: Vec<Lit> = explain.enablers().keys().copied().collect();

    let mut solution_history: Vec<IntermediateResult> = Default::default();
    let mut best_value: Option<IntCst> = None;
    let mut best_solution: Option<Solution> = None;

    let solver = explain.get_inner_mut();
    let result = solver.minimize_with_assumptions(
        objective,
        &enabler_lits,
        deadline,
        |sol: &Solution| {
            let obj = sol.lb(objective);
            println!("New solution with objective: {obj}");
            solution_history.push(IntermediateResult {
                timestamp: start_time.elapsed(),
                objective: obj as i64,
            });
            best_value = Some(obj);
            best_solution = Some(sol.clone());
        },
    );

    let status = match result {
        Ok(Ok(_sol)) => {
            println!("> OPTIMAL (cost: {})", best_value.unwrap_or(0));
            SolveStatus::Solved
        }
        Ok(Err(_unsat)) => {
            println!("> UNSATISFIABLE");
            SolveStatus::SolvedUnsat
        }
        Err(Exit::Interrupted) => {
            if best_solution.is_some() {
                println!("> TIMEOUT (best solution cost: {})", best_value.unwrap());
            } else {
                println!("> TIMEOUT (no solution found)");
            }
            SolveStatus::Timeout
        }
    };

    solver.print_stats();
    println!();

    if let Some(ref sol) = best_solution {
        let p = encoding.plan(sol);
        println!("==== Plan ====\n\n{p}");
        p.write_to_file(args.output.as_ref())?;
    }

    if let Some(report_dir) = args.report.as_ref() {
        let result = aries_bench_data::SolveResult {
            problem: aries_bench_data::Problem {
                name: args.problem.to_string_lossy().to_string(),
                timeout: args
                    .timeout
                    .map(|t| Duration::from_secs(t as u64))
                    .unwrap_or(Duration::MAX),
                flags: Default::default(),
            },
            status,
            runtime: start_time.elapsed(),
            objective_value: best_value.map(|v| v as i64),
            metrics: Default::default(),
            objective_history: solution_history,
        }
        .with_metric(SolverMetric::NumConflicts, solver.stats.num_conflicts as f64)
        .with_metric(SolverMetric::NumDecisions, solver.stats.num_decisions as f64)
        .with_metric(SolverMetric::NumDomUpdates, solver.stats.num_dom_updates as f64);

        result
            .save_to_dir(&report_dir.to_string_lossy())
            .map_err(|e| planx::Message::error(format!("{e}")))?;
    }

    println!("TOTAL RUNTIME: {:.6}", start_time.elapsed().as_secs_f64());
    Ok(())
}

fn parse_problem(args: &Args) -> Res<(pddl::Domain, pddl::Problem)> {
    let pb_path = &args.problem;
    if !pb_path.exists() {
        return planx::Message::error(format!("Problem file not found: {}", pb_path.display())).failed();
    }
    let dom_path = match &args.domain {
        Some(p) => p.clone(),
        None => pddl::find_domain_of(pb_path)?,
    };
    let dom = pddl::parse_pddl_domain(Input::from_file(&dom_path)?)?;
    let pb = pddl::parse_pddl_problem(Input::from_file(pb_path)?)?;
    Ok((dom, pb))
}
