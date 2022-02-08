use crate::encode::{encode, populate_with_task_network, populate_with_template_instances};
use crate::fmt::{format_hddl_plan, format_partial_plan, format_pddl_plan};
use crate::forward_search::ForwardSearcher;
use crate::solver::Strat::{Activity, Forward};
use crate::Solver;

use anyhow::{Context, Result};
use aries_model::state::Domains;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use structopt::StructOpt;

use aries_model::extensions::SavedAssignment;
use aries_planning::chronicles::analysis::hierarchical_is_non_recursive;
use aries_planning::chronicles::Problem;
use aries_planning::chronicles::*;

use aries_solver::parallel_solver::ParSolver;
use aries_tnet::theory::{StnConfig, StnTheory, TheoryPropagationLevel};

/// An automated planner for PDDL and HDDL problems.
#[derive(Debug, Default, Clone, StructOpt)]
#[structopt(name = "grpc", rename_all = "kebab-case")]
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

pub struct Planner {
    pub option: Opt,
    pub problem: Option<FiniteProblem>,
    pub plan: Option<Arc<Domains>>,
    pub start: Instant,
    pub end: Instant,
    pub htn_mode: bool,
}

impl Planner {
    pub fn new(option: Opt) -> Self {
        Self {
            option,
            problem: None,
            plan: None,
            start: Instant::now(),
            end: Instant::now(),
            htn_mode: false,
        }
    }

    pub fn solve(&mut self, mut spec: Problem, opt: &Opt) -> Result<()> {
        let mut result = None;

        println!("===== Preprocessing ======");
        aries_planning::chronicles::preprocessing::preprocess(&mut spec);
        println!("==========================");

        // if not explicitly given, compute the min/max search depth
        let max_depth = opt.max_depth.unwrap_or(u32::MAX);
        let min_depth = if let Some(min_depth) = opt.min_depth {
            min_depth
        } else if self.htn_mode && hierarchical_is_non_recursive(&spec) {
            max_depth // non recursive htn: bounded size, go directly to max
        } else {
            0
        };

        self.start = Instant::now();
        let mut pb = FiniteProblem {
            model: spec.context.model.clone(),
            origin: spec.context.origin(),
            horizon: spec.context.horizon(),
            chronicles: spec.chronicles.clone(),
            tables: spec.context.tables.clone(),
        };
        for n in min_depth..=max_depth {
            let depth_string = if n == u32::MAX {
                "∞".to_string()
            } else {
                n.to_string()
            };
            println!("{} Solving with {} actions", depth_string, depth_string);
            if self.htn_mode {
                populate_with_task_network(&mut pb, &spec, n)?;
            } else {
                populate_with_template_instances(&mut pb, &spec, |_| Some(n))?;
            }
            println!("  [{:.3}s] Populated", self.start.elapsed().as_secs_f32());
            if opt.no_search {
                propagate_and_print(&pb);
                break;
            } else {
                result = _solve(&pb, opt, self.htn_mode);
                println!("  [{:.3}s] Solved", self.start.elapsed().as_secs_f32());
            }
            if result.is_some() {
                break;
            }
        }
        self.problem = Some(pb.clone());
        self.end = Instant::now();
        self.plan = result;
        Ok(())
    }

    pub fn format_plan(&self, plan: &Option<Arc<Domains>>) -> Result<()> {
        if let Some(x) = plan {
            // println!("  Solution found");
            let plan = if self.htn_mode {
                format!(
                    "\n**** Decomposition ****\n\n\
                        {}\n\n\
                        **** Plan ****\n\n\
                        {}",
                    format_hddl_plan(
                        &self
                            .problem
                            .clone()
                            .with_context(|| "Unable to format HDDL problem. Formatting failed".to_string())?,
                        x
                    )?,
                    format_pddl_plan(
                        &self
                            .problem
                            .clone()
                            .with_context(|| "Unable to format PDDL problem. Formatting failed".to_string())?,
                        x
                    )?
                )
            } else {
                format!(
                    "\n**** Plan ****\n\n\
                        {}",
                format_pddl_plan(
                    &self
                        .problem
                        .clone()
                        .with_context(|| "Unable to format PDDL problem. Formatting failed".to_string())?,
                    x,
                )?)
            };
            println!("{}", plan);
            if let Some(plan_out_file) = self.option.plan_out_file.clone() {
                let mut file = File::create(plan_out_file)?;
                file.write_all(plan.as_bytes())?;
            }
            Ok(())
        } else {
            println!("  No solution found");
            Ok(())
        }
    }
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

fn _solve(pb: &FiniteProblem, opt: &Opt, htn_mode: bool) -> Option<std::sync::Arc<SavedAssignment>> {
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
        let res = solver.minimize(pb.horizon.num).unwrap();
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
