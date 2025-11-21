use anyhow::{Context, Result};
use aries::core::state::Domains;
use aries::utils::input::Input;
use aries_planners::solver::{format_plan, solve, SolverResult};
use aries_planners::solver::{Metric, Strat};
use aries_planning::chronicles::analysis::hierarchy::hierarchical_is_non_recursive;
use aries_planning::chronicles::FiniteProblem;
use aries_planning::parsing::pddl::{find_domain_of, parse_pddl_domain, parse_pddl_problem, PddlFeature};
use aries_planning::parsing::pddl_to_chronicles;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use structopt::StructOpt;

/// An automated planner for PDDL and HDDL problems.
#[derive(Debug, Clone, StructOpt)]
#[structopt(name = "aries-plan", rename_all = "kebab-case")]
pub struct Opt {
    /// Path to the domain file. If absent, aries will try to infer it from conventions.
    #[structopt(long, short)]
    domain: Option<PathBuf>,
    /// Path to the problem file.
    problem: PathBuf,

    /// If set, a machine readable plan will be written to the file upon termination.
    /// See the `--anytime` option to also write intermediate files.
    #[structopt(long = "output", short = "o")]
    plan_out_file: Option<PathBuf>,

    /// Minimum depth of the instantiation. (depth of HTN tree or number of standalone actions with the same name).
    #[structopt(long)]
    min_depth: Option<u32>,
    /// Maximum depth of instantiation
    #[structopt(long)]
    max_depth: Option<u32>,

    /// If set, the solver will attempt to optimize a particular metric, until a proven optimal solution is found.
    /// Possible values: "makespan", "plan-length", "action-costs"
    #[structopt(long = "optimize")]
    optimize: Option<Metric>,
    /// Indicates the optimization value to "beat" (be better than)
    #[structopt(long)]
    metric_bound: Option<i32>,

    /// When used in conjunction with `--output`, each plan found will be written to the output file.
    /// The previous plan, if any will be overwritten.
    #[structopt(long = "anytime")]
    anytime: bool,

    /// If provided, the solver will only run the specified strategy instead of default set of strategies.
    /// When repeated, several strategies will be run in parallel.
    #[structopt(long = "strategy", short = "s")]
    strategies: Vec<Strat>,

    /// Logging level to use: one of "error", "warn", "info", "debug", "trace"
    #[structopt(short, long, default_value = "info")]
    log_level: tracing::Level,

    /// Indicates that the problem is known to be solvable.
    /// If set, the planner will exit with a non-zero exit code if it proves unsatifiability
    #[structopt(long)]
    sat: bool,

    /// Indicates that the problem is known to be unsolvable.
    /// If set, the planner will exit with a non-zero exit code if it found a solution
    #[structopt(long)]
    unsat: bool,
}

fn main() -> Result<()> {
    // Terminate the process if a thread panics.
    // take_hook() returns the default hook in case when a custom one is not set
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        std::process::exit(1);
    }));

    let opt: Opt = Opt::from_args();

    // set up logger
    let subscriber = tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::Uptime::from(std::time::Instant::now()))
        // .without_time() // if activated, no time will be printed on logs (useful for counting events with `counts`)
        .with_thread_ids(true)
        .with_max_level(opt.log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let problem_file = &opt.problem;
    anyhow::ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match opt.domain {
        Some(ref name) => name.clone(),
        None => find_domain_of(&problem_file).context("Consider specifying the domain with the option -d/--domain")?,
    };

    let dom = Input::from_file(&domain_file)?;
    let prob = Input::from_file(&problem_file)?;

    let dom = parse_pddl_domain(dom)?;
    let prob = parse_pddl_problem(prob)?;
    let warm_up_plan = None;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    // true if we are doing HTN planning, false otherwise
    let htn_mode = dom.features.contains(&PddlFeature::Hierarchy);

    // if not explicitly given, compute the min/max search depth
    let max_depth = opt.max_depth.unwrap_or(u32::MAX);
    let min_depth = if let Some(min_depth) = opt.min_depth {
        min_depth
    } else if htn_mode && hierarchical_is_non_recursive(&spec) {
        max_depth // non recursive htn: bounded size, go directly to max
    } else {
        0
    };

    // prints a plan to a standard output and to the provided file, if any
    let print_plan = move |finite_problem: &FiniteProblem, assignment: &Domains, output_file: Option<&PathBuf>| {
        if let Ok(plan_out) = format_plan(finite_problem, assignment, htn_mode) {
            println!("\n{plan_out}");

            // Write the output to a file if requested
            if let Some(plan_out_file) = output_file {
                if let Ok(mut file) = File::create(plan_out_file) {
                    let _ = file.write_all(plan_out.as_bytes());
                }
            }
        } else {
            tracing::error!("Problem while formatting plan.")
        }
    };
    let anytime_out_file = if opt.anytime { opt.plan_out_file.clone() } else { None };
    let result = solve(
        spec,
        min_depth,
        max_depth,
        &opt.strategies,
        opt.optimize,
        htn_mode,
        warm_up_plan,
        |pb, sol| print_plan(pb, &sol, anytime_out_file.as_ref()),
        None,
        opt.metric_bound,
        true,
    )?;

    match result {
        SolverResult::Sol((finite_problem, assignment)) => {
            print_plan(&finite_problem, &assignment, opt.plan_out_file.as_ref());
            anyhow::ensure!(!opt.unsat, "Solution found to an unsat problem.");
        }
        SolverResult::Unsat(_) => {
            println!("\nNo plan found");
            anyhow::ensure!(!opt.sat, "No solution found to a solvable pproblem.");
        }
        SolverResult::Timeout(_) => println!("\nTimeout"),
    }

    Ok(())
}
