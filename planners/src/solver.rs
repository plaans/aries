use crate::encode::{encode, populate_with_task_network, populate_with_template_instances};
use crate::fmt::{format_hddl_plan, format_pddl_plan};
use crate::forward_search::ForwardSearcher;
use crate::solver::Strat::{Activity, Forward};
use crate::Solver;

use anyhow::Result;
use aries_core::state::Domains;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use aries_model::extensions::SavedAssignment;
use aries_planning::chronicles::Problem;
use aries_planning::chronicles::*;

use aries_solver::parallel_solver::ParSolver;
use aries_tnet::theory::{StnConfig, StnTheory, TheoryPropagationLevel};

pub fn deepening_solve(
    mut spec: Problem,
    min_depth: u32,
    max_depth: u32,
    strategies: &[Strat],
    optimize_makespan: bool,
    htn_mode: bool,
) -> Result<(FiniteProblem, Option<Arc<Domains>>)> {
    let mut result = None;
    let mut problem = None;

    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(&mut spec);
    println!("==========================");

    let start = Instant::now();
    for n in min_depth..=max_depth {
        let mut pb = FiniteProblem {
            model: spec.context.model.clone(),
            origin: spec.context.origin(),
            horizon: spec.context.horizon(),
            chronicles: spec.chronicles.clone(),
            tables: spec.context.tables.clone(),
        };
        let depth_string = if n == u32::MAX {
            "∞".to_string()
        } else {
            n.to_string()
        };
        println!("{} Solving with {} actions", depth_string, depth_string);
        if htn_mode {
            populate_with_task_network(&mut pb, &spec, n)?;
        } else {
            populate_with_template_instances(&mut pb, &spec, |_| Some(n))?;
        }
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        result = solve_finite_problem(&pb, strategies, optimize_makespan, htn_mode);
        println!("  [{:.3}s] Solved", start.elapsed().as_secs_f32());

        if result.is_some() {
            problem = Some(pb);
            break;
        }
    }
    Ok((problem.unwrap(), result))
}

pub fn format_plan(problem: &FiniteProblem, plan: &Option<Arc<Domains>>, htn_mode: bool) -> Result<String> {
    if let Some(x) = plan {
        let plan = if htn_mode {
            format!(
                "\n**** Decomposition ****\n\n\
                        {}\n\n\
                        **** Plan ****\n\n\
                        {}",
                format_hddl_plan(problem, x)?,
                format_pddl_plan(problem, x)?
            )
        } else {
            format_pddl_plan(problem, x)?
        };
        println!("\n**** Plan ****\n\n {}", plan);
        Ok(plan)
    } else {
        Ok("".to_string())
    }
}

pub fn init_solver(pb: &FiniteProblem) -> Box<Solver> {
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
pub enum Strat {
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

fn solve_finite_problem(
    pb: &FiniteProblem,
    strategies: &[Strat],
    optimize_makespan: bool,
    htn_mode: bool,
) -> Option<std::sync::Arc<SavedAssignment>> {
    let solver = init_solver(pb);
    let strats: &[Strat] = if !strategies.is_empty() {
        strategies
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

    let found_plan = if optimize_makespan {
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
