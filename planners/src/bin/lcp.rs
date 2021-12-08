use crate::Strat::{Activity, Forward};
use anyhow::{Context, Result};
use aries_model::extensions::SavedAssignment;
use aries_planners::encode::{encode, populate_with_task_network, populate_with_template_instances};
use aries_planners::fmt::{format_hddl_plan, format_partial_plan, format_pddl_plan};
use aries_planners::forward_search::ForwardSearcher;
use aries_planners::Solver;
use aries_planning::chronicles::analysis::hierarchical_is_non_recursive;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::{find_domain_of, parse_pddl_domain, parse_pddl_problem, PddlFeature};
use aries_planning::parsing::pddl_to_chronicles;
use aries_solver::parallel_solver::ParSolver;
use aries_tnet::theory::{StnConfig, StnTheory, TheoryPropagationLevel};
use aries_utils::input::Input;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use structopt::StructOpt;

/// An automated planner for PDDL and HDDL problems.
#[derive(Debug, StructOpt)]
#[structopt(name = "lcp", rename_all = "kebab-case")]
struct Opt {
    /// path to the domain file (if not provided, the solver will try to find it based on naming conventions).
    #[structopt(long, short)]
    domain: Option<PathBuf>,
    /// Path to the problem file.
    problem: PathBuf,
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

    // true if we are doing HTN planning, false otherwise
    let htn_mode = dom.features.contains(&PddlFeature::Hierarchy);

    let mut spec = pddl_to_chronicles(&dom, &prob)?;

    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(&mut spec);
    println!("==========================");

    // if not explicitly given, compute the min/max search depth
    let max_depth = opt.max_depth.unwrap_or(u32::MAX);
    let min_depth = if let Some(min_depth) = opt.min_depth {
        min_depth
    } else if htn_mode && hierarchical_is_non_recursive(&spec) {
        max_depth // non recursive htn: bounded size, go directly to max
    } else {
        0
    };

    for n in min_depth..=max_depth {
        let depth_string = if n == u32::MAX {
            "∞".to_string()
        } else {
            n.to_string()
        };
        println!("{} Solving with {} actions", depth_string, depth_string);
        let start = Instant::now();
        let mut pb = FiniteProblem {
            model: spec.context.model.clone(),
            origin: spec.context.origin(),
            horizon: spec.context.horizon(),
            chronicles: spec.chronicles.clone(),
            tables: spec.context.tables.clone(),
        };
        if htn_mode {
            populate_with_task_network(&mut pb, &spec, n)?;
        } else {
            populate_with_template_instances(&mut pb, &spec, |_| Some(n))?;
        }
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let start = Instant::now();
        if opt.no_search {
            propagate_and_print(&pb);
            break;
        } else {
            let result = solve(&pb, &opt, htn_mode);
            println!("  [{:.3}s] solved", start.elapsed().as_secs_f32());
            if let Some(x) = result {
                // println!("{}", format_partial_plan(&pb, &x)?);
                println!("  Solution found");
                let plan = if htn_mode {
                    format_hddl_plan(&pb, &x)?
                } else {
                    format_pddl_plan(&pb, &x)?
                };
                println!("{}", plan);
                if let Some(plan_out_file) = opt.plan_out_file {
                    let mut file = File::create(plan_out_file)?;
                    file.write_all(plan.as_bytes())?;
                }
                break;
            }
        }
    }

    Ok(())
}

fn init_solver(pb: &FiniteProblem) -> Box<Solver> {
    let model = encode(pb).unwrap(); // TODO: report error
    let stn_config = StnConfig {
        theory_propagation: TheoryPropagationLevel::Full,
        ..Default::default()
    };

    let mut solver = Box::new(aries_solver::solver::Solver::new(model));
    solver.add_theory(|tok| StnTheory::new(tok, stn_config));
    solver
}

/// Default set of strategies for HTN problems
const HTN_DEFAULT_STRATEGIES: [Strat; 2] = [Strat::Activity, Strat::Forward];
/// Default set of strategies for generative (flat) problems.
const GEN_DEFAULT_STRATEGIES: [Strat; 1] = [Strat::Activity];

#[derive(Copy, Clone, Debug)]
enum Strat {
    /// Activity based search
    Activity,
    /// Mimics forward search in HTN problems.
    Forward,
}

impl Strat {
    /// Configure the given solver to follow the strategy.
    pub fn adapt_solver(self, solver: &mut Solver, problem: &FiniteProblem) {
        match self {
            Activity => {
                // nothing, activity based search is the default configuration
            }
            Forward => solver.set_brancher(ForwardSearcher::new(Arc::new(problem.clone()))),
        }
    }
}

impl FromStr for Strat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" | "act" | "activity" => Ok(Activity),
            "2" | "fwd" | "forward" => Ok(Forward),
            _ => Err(format!("Unknown search strategy: {}", s)),
        }
    }
}

fn solve(pb: &FiniteProblem, opt: &Opt, htn_mode: bool) -> Option<std::sync::Arc<SavedAssignment>> {
    let solver = init_solver(pb);
    let strats: &[Strat] = if !opt.strategies.is_empty() {
        &opt.strategies
    } else if htn_mode {
        &HTN_DEFAULT_STRATEGIES
    } else {
        &GEN_DEFAULT_STRATEGIES
    };
    let mut solver = if htn_mode {
        aries_solver::parallel_solver::ParSolver::new(solver, strats.len(), |id, s| strats[id].adapt_solver(s, pb))
    } else {
        ParSolver::new(solver, 1, |_, _| {})
    };

    let found_plan = if opt.optimize_makespan {
        let res = solver.minimize(pb.horizon).unwrap();
        res.map(|tup| tup.1)
    } else {
        solver.solve().unwrap()
    };

    if let Some(solution) = found_plan {
        solver.print_stats();
        Some(solution)
    } else {
        None
    }
}

fn propagate_and_print(pb: &FiniteProblem) {
    let mut solver = init_solver(pb);
    if solver.propagate_and_backtrack_to_consistent() {
        let str = format_partial_plan(pb, &solver.model).unwrap();
        println!("{}", str);
    } else {
        panic!("Invalid problem");
    }
}
