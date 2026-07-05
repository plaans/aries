mod problem;

use aries_solver::prelude::*;
use aries_solver::solver::{Exit, SearchLimit};
use aries_bench_data::IntermediateResult;
use aries_lprelax::LpRelax;
use clap::Parser;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::problem::IlpProblem;

type Model = aries_solver::model::Model<String>;
type Solver = aries_solver::solver::Solver<String>;

#[derive(Parser)]
#[command(version, about, name = "aries-ilp")]
struct Cli {
    file: PathBuf,

    /// Don't use LP relaxation
    #[arg(long, default_value_t = false)]
    no_lprelax: bool,

    /// Timeout (seconds)
    #[arg(long, short)]
    timeout: Option<u64>,

    #[arg(long)]
    expected_objective: Option<IntCst>,

    #[arg(long, default_value_t = false)]
    expected_unsat: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    assert!(
        !(cli.expected_objective.is_some() && cli.expected_unsat),
        "Cannot expect UNSAT and an objective value at the same time."
    );

    let problem_string = std::fs::read_to_string(&cli.file)?;
    let use_lprelax = !cli.no_lprelax;

    let problem_info = aries_bench_data::Problem {
        name: cli.file.to_string_lossy().to_string(),
        timeout: Duration::from_secs(cli.timeout.unwrap_or_default()),
        flags: BTreeMap::new(),
    };

    let problem = match &cli.file.extension().and_then(std::ffi::OsStr::to_str) {
        Some("mps") => IlpProblem::from_mps(&problem_string)?,
        Some("lp") => IlpProblem::from_lp(&problem_string)?,
        _ => return Err(anyhow::anyhow!("Input file needs to be .mps or .lp")),
    };
    // println!("{:#?}", problem);

    let model = problem.encode_model()?;

    let mut solver = make_solver(&problem, model, use_lprelax);

    solve(
        &problem,
        &mut solver,
        cli.timeout,
        cli.expected_objective,
        cli.expected_unsat,
        problem_info,
    )
}

fn solve(
    problem: &IlpProblem,
    solver: &mut Solver,
    timeout: Option<u64>,
    expected_objective: Option<IntCst>,
    expected_unsat: bool,
    problem_info: aries_bench_data::Problem,
) -> anyhow::Result<()> {
    let start_time = Instant::now();
    let deadline = timeout
        .map(|dur| SearchLimit::Deadline(start_time + Duration::from_secs(dur)))
        .unwrap_or(SearchLimit::None);

    let objective = problem
        .obj
        .as_ref()
        .and_then(|(obj_name, _)| solver.model.shape.get_variable(obj_name))
        .unwrap_or(Var::ZERO.into());

    let mut solution_history: Vec<IntermediateResult> = Default::default();
    let mut best = Option::None;

    let on_new_solution = |obj: IntCst, sol: &Solution| {
        println!("New solution with objective: {}", obj);
        solution_history.push(IntermediateResult {
            timestamp: start_time.elapsed(),
            objective: obj as i64,
        });
        best = Some(sol.clone());
    };

    let result = match problem.sense {
        lp_parser_rs::model::Sense::Minimize => solver.minimize_with_callback(objective, on_new_solution, deadline),
        lp_parser_rs::model::Sense::Maximize => solver.maximize_with_callback(objective, on_new_solution, deadline),
    };
    solver.print_stats();
    println!();

    let status = match result {
        Ok(Some((obj, solution))) => {
            let optimum = solution.value_of(objective).unwrap();
            assert_eq!(obj, optimum); // sanity check
            println!("> OPTIMAL (objective: {optimum})");

            assert!(!expected_unsat, "Expected an unsatisfiable problem");
            if let Some(expected) = expected_objective {
                assert_eq!(
                    optimum, expected,
                    "The objective found ({optimum}) is not the expected one ({expected})"
                );
            }
            println!(
                "XX\t{}\t{}\t{}",
                problem_info.name,
                optimum,
                start_time.elapsed().as_secs_f64()
            );
            aries_bench_data::SolveStatus::Solved
        }
        Ok(None) => {
            println!("> UNSATISFIABLE");
            assert!(expected_objective.is_none(), "Expected a valid solution");
            aries_bench_data::SolveStatus::Solved
        }
        Err(Exit::Interrupted) => match best.as_ref() {
            Some(sol) => {
                let best_cost = sol.value_of(objective).unwrap();
                println!("> TIMEOUT (best solution cost {best_cost})");
                aries_bench_data::SolveStatus::Timeout
            }
            None => {
                println!("> TIMEOUT (no solution found)");
                aries_bench_data::SolveStatus::Timeout
            }
        },
    };

    //let result = aries_bench_data::SolveResult {
    aries_bench_data::SolveResult {
        problem: problem_info,
        status,
        runtime: start_time.elapsed(),
        objective_value: best.map(|sol| sol.value_of(objective).unwrap() as i64),
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

    println!("TOTAL RUNTIME: {:.6}", start_time.elapsed().as_secs_f64());
    Ok(())
}

fn make_solver(problem: &IlpProblem, model: Model, use_lp_relax: bool) -> Solver {
    let extra_reasoners: Vec<Box<dyn aries_solver::reasoners::Theory>> = if use_lp_relax {
        let mut lprelax = LpRelax::default();

        let mut var_name_to_col_map = HashMap::new();

        for (name, (lb, ub)) in &problem.vars {
            let col = lprelax.add_column(Some((*lb).into()), Some((*ub).into()));

            let var = model.shape.get_variable(name).unwrap();
            var_name_to_col_map.insert(name.clone(), col);

            lprelax.add_var_half_binding_default(var, col);
            lprelax.add_col_half_binding_default(col, var);
        }
        for (row_coefs, lb, ub) in problem.constrs.values() {
            let row_coefs = row_coefs
                .iter()
                .map(|(name, coef)| (*var_name_to_col_map.get(name).unwrap(), (*coef).into()));
            lprelax.add_row(row_coefs, Some((*lb).into()), Some((*ub).into()));
        }

        if let Some((obj_name, obj_coefs)) = &problem.obj {
            let obj_coefs = obj_coefs
                .iter()
                .map(|(name, coef)| (*var_name_to_col_map.get(name).unwrap(), (*coef).into()));

            let obj_var = model.shape.get_variable(obj_name).unwrap();
            let obj_col = lprelax.add_objective_column(
                obj_var,
                obj_coefs,
                match problem.sense {
                    lp_parser_rs::model::Sense::Minimize => aries_lprelax::LpObjectiveSense::Minimise,
                    lp_parser_rs::model::Sense::Maximize => aries_lprelax::LpObjectiveSense::Maximise,
                },
            );
            lprelax.add_var_half_binding_default(obj_var, obj_col);
            lprelax.add_col_half_binding_default(obj_col, obj_var);
        }
        vec![Box::new(lprelax)]
    } else {
        vec![]
    };

    Solver::with_extra_reasoners(model, extra_reasoners)
}
