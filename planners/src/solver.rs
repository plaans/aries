use crate::encode::{encode, populate_with_task_network, populate_with_template_instances};
use crate::fmt::{format_hddl_plan, format_partial_plan, format_pddl_plan};
use crate::forward_search::ForwardSearcher;
use crate::solver::Strat::{Activity, Forward};
use crate::Solver;
use anyhow::Result;
use aries_core::state::Domains;
use aries_model::extensions::SavedAssignment;
use aries_planning::chronicles::Problem;
use aries_planning::chronicles::*;
use aries_tnet::theory::{StnConfig, StnTheory, TheoryPropagationLevel};
use env_param::EnvParam;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

/// If set to true, prints the result of the initial propagation at each depth.
static PRINT_INITIAL_PROPAGATION: EnvParam<bool> = EnvParam::new("ARIES_PRINT_INITIAL_PROPAGATION", "false");

/// Search for plan based on the `base_problem`.
///
/// The solver will look for plan by generating subproblem of increasing `depth`
/// (for `depth` in `{min_depth, max_depth]`) where `depth` defines the number of allowed actions
/// in the subproblem.
///
/// The `depth` parameter is increased until a plan is found or foes over `max_depth`.
///
/// When a plan is found, the solver returns the corresponding subproblem and the instantiation of
/// its variables.
pub fn solve(
    mut base_problem: Problem,
    min_depth: u32,
    max_depth: u32,
    strategies: &[Strat],
    optimize_makespan: bool,
    htn_mode: bool,
) -> Result<Option<(FiniteProblem, Arc<Domains>)>> {
    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(&mut base_problem);
    println!("==========================");

    let start = Instant::now();
    for depth in min_depth..=max_depth {
        let mut pb = FiniteProblem {
            model: base_problem.context.model.clone(),
            origin: base_problem.context.origin(),
            horizon: base_problem.context.horizon(),
            chronicles: base_problem.chronicles.clone(),
        };
        let depth_string = if depth == u32::MAX {
            "∞".to_string()
        } else {
            depth.to_string()
        };
        println!("{} Solving with {} actions", depth_string, depth_string);
        if htn_mode {
            populate_with_task_network(&mut pb, &base_problem, depth)?;
        } else {
            populate_with_template_instances(&mut pb, &base_problem, |_| Some(depth))?;
        }
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let result = solve_finite_problem(&pb, strategies, optimize_makespan, htn_mode);
        println!("  [{:.3}s] Solved", start.elapsed().as_secs_f32());

        if let Some(result) = result {
            // we got a valid assignment, return the corresponding plan
            return Ok(Some((pb, result)));
        }
    }
    Ok(None)
}

/// This function mimics the instantiation of the subproblem, run the propagation and prints the result.
/// and exits immediately.
///
/// Note that is meant to facilitate debugging of the planner during development.
///
/// Returns true if the propagation succeeded.
fn propagate_and_print(pb: &FiniteProblem) -> bool {
    // for ch in &pb.chronicles {
    //     Printer::print_chronicle(&ch.chronicle, &pb.model);
    // }

    let mut solver = init_solver(pb);

    println!("\n======== BEFORE INITIAL PROPAGATION ======\n");
    let str = format_partial_plan(pb, &solver.model).unwrap();
    println!("{}", str);

    println!("\n======== AFTER INITIAL PROPAGATION ======\n");
    if solver.propagate_and_backtrack_to_consistent() {
        let str = format_partial_plan(pb, &solver.model).unwrap();
        println!("{}", str);
        true
    } else {
        println!("==> Propagation failed.");
        false
    }
}

pub fn format_plan(problem: &FiniteProblem, plan: &Arc<Domains>, htn_mode: bool) -> Result<String> {
    let plan = if htn_mode {
        format!(
            "\n**** Decomposition ****\n\n\
             {}\n\n\
             **** Plan ****\n\n\
             {}",
            format_hddl_plan(problem, plan)?,
            format_pddl_plan(problem, plan)?
        )
    } else {
        format_pddl_plan(problem, plan)?
    };
    Ok(plan)
}

pub fn init_solver(pb: &FiniteProblem) -> Box<Solver> {
    let model = encode(pb).expect("Failed to encode the problem"); // TODO: report error
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

/// Instantiates a solver for the given subproblem and attempts to solve it.
///
/// If more than one strategy is given, each strategy will have its own solver run on a dedicated thread.
/// If no strategy is given, then a default set of strategies will be automatically selected.
///
/// If a valid solution of the subproblem is found, the solver will return a satisfying assignment.
fn solve_finite_problem(
    pb: &FiniteProblem,
    strategies: &[Strat],
    optimize_makespan: bool,
    htn_mode: bool,
) -> Option<std::sync::Arc<SavedAssignment>> {
    if PRINT_INITIAL_PROPAGATION.get() {
        propagate_and_print(pb);
    }
    let solver = init_solver(pb);

    // select the set of strategies, based on user-input or hard-coded defaults.
    let strats: &[Strat] = if !strategies.is_empty() {
        strategies
    } else if htn_mode {
        &HTN_DEFAULT_STRATEGIES
    } else {
        &GEN_DEFAULT_STRATEGIES
    };
    let mut solver =
        aries_solver::parallel_solver::ParSolver::new(solver, strats.len(), |id, s| strats[id].adapt_solver(s, pb));

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
