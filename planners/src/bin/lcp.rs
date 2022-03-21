use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use aries_planners::fmt::format_partial_plan;
use aries_planning::chronicles::analysis::hierarchical_is_non_recursive;
use aries_planning::chronicles::{FiniteProblem, Problem};
use structopt::StructOpt;

use aries_planners::solver::Strat;
use aries_planners::solver::{deepening_solve, format_plan, init_solver};
use aries_planning::parsing::pddl::{find_domain_of, parse_pddl_domain, parse_pddl_problem, PddlFeature};
use aries_planning::parsing::pddl_to_chronicles;
use aries_utils::input::Input;

/// An automated planner for PDDL and HDDL problems.
#[derive(Debug, Default, Clone, StructOpt)]
#[structopt(name = "lcp", rename_all = "kebab-case")]
pub struct Opt {
    #[structopt(long, short)]
    pub domain: Option<PathBuf>,
    /// Path to the problem file.
    pub problem: PathBuf,
    /// If set, a machine readable plan will be written to the file.
    #[structopt(long = "output", short = "o")]
    plan_out_file: Option<PathBuf>,
    /// Minimum depth of the instantiation. (depth of HTN tree or number of standalone actions with the same name).
    #[structopt(long)]
    min_depth: Option<u32>,
    /// Maximum depth of instantiation
    #[structopt(long)]
    max_depth: Option<u32>,
    /// If set, the solver will attempt to minimize the makespan of the plan.
    #[structopt(long = "optimize")]
    optimize_makespan: bool,
    /// If true, then the problem will be constructed, a full propagation will be made and the resulting
    /// partial plan will be displayed.
    #[structopt(long = "no-search")]
    no_search: bool,
    /// If provided, the solver will only run the specified strategy instead of default set of strategies.
    /// When repeated, several strategies will be run in parallel.
    #[structopt(long = "strategy", short = "s")]
    strategies: Vec<Strat>,
}

fn propagate_and_print(spec: &Problem) {
    let pb = FiniteProblem {
        model: spec.context.model.clone(),
        origin: spec.context.origin(),
        horizon: spec.context.horizon(),
        chronicles: spec.chronicles.clone(),
        tables: spec.context.tables.clone(),
    };
    let mut solver = init_solver(&pb);
    if solver.propagate_and_backtrack_to_consistent() {
        let str = format_partial_plan(&pb, &solver.model).unwrap();
        println!("{}", str);
    } else {
        panic!("Invalid problem");
    }
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

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

    if opt.no_search {
        // TODO: the propagation is not done here yet
        propagate_and_print(&spec);
    }

    let (finite_problem, plan) = deepening_solve(
        spec,
        min_depth,
        max_depth,
        &opt.strategies,
        opt.optimize_makespan,
        htn_mode,
    )?;
    if plan.is_some() {
        let plan_out = format_plan(&finite_problem, &plan, htn_mode)?;

        // Write the output to a file if requested
        if let Some(plan_out_file) = opt.plan_out_file.clone() {
            let mut file = File::create(plan_out_file)?;
            file.write_all(plan_out.as_bytes())?;
        }
    } else {
        println!("\nNo plan found");
    }

    Ok(())
}
