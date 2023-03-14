use crate::encode::{encode, populate_with_task_network, populate_with_template_instances};
use crate::fmt::{format_hddl_plan, format_partial_plan, format_pddl_plan};
use crate::forward_search::ForwardSearcher;
use crate::Solver;
use anyhow::Result;
use aries::core::state::Domains;
use aries::core::VarRef;
use aries::model::extensions::SavedAssignment;
use aries::model::lang::IAtom;
use aries::reasoners::stn::theory::{StnConfig, TheoryPropagationLevel};
use aries::solver::parallel::Solution;
use aries::solver::search::activity::*;
use aries_planning::chronicles::Problem;
use aries_planning::chronicles::*;
use env_param::EnvParam;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

/// If set to true, prints the result of the initial propagation at each depth.
static PRINT_INITIAL_PROPAGATION: EnvParam<bool> = EnvParam::new("ARIES_PRINT_INITIAL_PROPAGATION", "false");

pub type SolverResult<Sol> = aries::solver::parallel::SolverResult<Sol>;

#[derive(Copy, Clone, Debug)]
pub enum Metric {
    Makespan,
    /// Number of actions in the plan
    PlanLength,
    /// Sum of all chronicle costs
    ActionCosts,
}

impl FromStr for Metric {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "makespan" | "duration" => Ok(Metric::Makespan),
            "plan-length" | "length" => Ok(Metric::PlanLength),
            "action-costs" | "costs" => Ok(Metric::ActionCosts),
            _ => Err(format!(
                "Unknown metric: '{s}'. Valid options are: 'makespan', 'plan-length', 'action-costs"
            )),
        }
    }
}

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
#[allow(clippy::too_many_arguments)]
pub fn solve(
    mut base_problem: Problem,
    min_depth: u32,
    max_depth: u32,
    strategies: &[Strat],
    metric: Option<Metric>,
    htn_mode: bool,
    on_new_sol: impl Fn(&FiniteProblem, Arc<SavedAssignment>) + Clone,
    deadline: Option<Instant>,
) -> Result<SolverResult<(Arc<FiniteProblem>, Arc<Domains>)>> {
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
        println!("{depth_string} Solving with {depth_string} actions");
        if htn_mode {
            populate_with_task_network(&mut pb, &base_problem, depth)?;
        } else {
            populate_with_template_instances(&mut pb, &base_problem, |_| Some(depth))?;
        }
        let pb = Arc::new(pb);

        let on_new_valid_assignment = {
            let pb = pb.clone();
            let on_new_sol = on_new_sol.clone();
            move |ass: Arc<SavedAssignment>| on_new_sol(&pb, ass)
        };
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let result = solve_finite_problem(&pb, strategies, metric, htn_mode, on_new_valid_assignment, deadline);
        println!("  [{:.3}s] Solved", start.elapsed().as_secs_f32());

        let result = result.map(|assignment| (pb, assignment));
        match result {
            SolverResult::Unsat => {} // continue (increase depth)
            other => return Ok(other),
        }
    }
    Ok(SolverResult::Unsat)
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

    let (mut solver, _) = init_solver(pb, None);

    println!("\n======== BEFORE INITIAL PROPAGATION ======\n");
    let str = format_partial_plan(pb, &solver.model).unwrap();
    println!("{str}");

    println!("\n======== AFTER INITIAL PROPAGATION ======\n");
    if solver.propagate_and_backtrack_to_consistent() {
        let str = format_partial_plan(pb, &solver.model).unwrap();
        println!("{str}");
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

pub fn init_solver(pb: &FiniteProblem, metric: Option<Metric>) -> (Box<Solver>, Option<IAtom>) {
    let (model, metric) = encode(pb, metric).expect("Failed to encode the problem"); // TODO: report error
    let stn_config = StnConfig {
        theory_propagation: TheoryPropagationLevel::Full,
        ..Default::default()
    };

    let mut solver = Box::new(aries::solver::Solver::new(model));
    solver.reasoners.diff.config = stn_config;
    (solver, metric)
}

/// Default set of strategies for HTN problems
const HTN_DEFAULT_STRATEGIES: [Strat; 3] = [Strat::Activity, Strat::Forward, Strat::ActivityNonTemporalFirst];
/// Default set of strategies for generative (flat) problems.
const GEN_DEFAULT_STRATEGIES: [Strat; 2] = [Strat::Activity, Strat::ActivityNonTemporalFirst];

#[derive(Copy, Clone, Debug)]
pub enum Strat {
    /// Activity based search
    Activity,
    /// An activity-based variable selection strategy that delays branching on temporal variables.
    ActivityNonTemporalFirst,
    /// Mimics forward search in HTN problems.
    Forward,
}

/// An activity-based variable selection heuristics that delays branching on temporal variables.
struct ActivityNonTemporalFirstHeuristic;
impl Heuristic<VarLabel> for ActivityNonTemporalFirstHeuristic {
    fn decision_stage(&self, _var: VarRef, label: Option<&VarLabel>, _model: &aries::model::Model<VarLabel>) -> u8 {
        match label.as_ref() {
            None => 0,
            Some(VarLabel(_, tpe)) => match tpe {
                VarType::Presence | VarType::Reification | VarType::Parameter(_) => 0,
                VarType::ChronicleStart | VarType::ChronicleEnd | VarType::TaskStart(_) | VarType::TaskEnd(_) => 1,
                VarType::Horizon | VarType::EffectEnd | VarType::Cost => 2,
            },
        }
    }
}

impl Strat {
    /// Configure the given solver to follow the strategy.
    pub fn adapt_solver(self, solver: &mut Solver, problem: &FiniteProblem) {
        match self {
            Strat::Activity => {
                // nothing, activity based search is the default configuration
            }
            Strat::ActivityNonTemporalFirst => {
                solver.set_brancher(ActivityBrancher::new_with_heuristic(ActivityNonTemporalFirstHeuristic))
            }
            Strat::Forward => solver.set_brancher(ForwardSearcher::new(Arc::new(problem.clone()))),
        }
    }
}

impl FromStr for Strat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" | "act" | "activity" => Ok(Strat::Activity),
            "2" | "fwd" | "forward" => Ok(Strat::Forward),
            "3" | "act-no-time" | "activity-no-time" => Ok(Strat::ActivityNonTemporalFirst),
            _ => Err(format!("Unknown search strategy: {s}")),
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
    metric: Option<Metric>,
    htn_mode: bool,
    on_new_solution: impl Fn(Arc<SavedAssignment>),
    deadline: Option<Instant>,
) -> SolverResult<Solution> {
    if PRINT_INITIAL_PROPAGATION.get() {
        propagate_and_print(pb);
    }
    let (solver, metric) = init_solver(pb, metric);

    // select the set of strategies, based on user-input or hard-coded defaults.
    let strats: &[Strat] = if !strategies.is_empty() {
        strategies
    } else if htn_mode {
        &HTN_DEFAULT_STRATEGIES
    } else {
        &GEN_DEFAULT_STRATEGIES
    };
    let mut solver =
        aries::solver::parallel::ParSolver::new(solver, strats.len(), |id, s| strats[id].adapt_solver(s, pb));

    let result = if let Some(metric) = metric {
        solver.minimize_with(metric, on_new_solution, deadline)
    } else {
        solver.solve(deadline)
    };

    if let SolverResult::Sol(_) = result {
        solver.print_stats()
    }
    result
}
