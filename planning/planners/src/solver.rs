use crate::encode::{encode, populate_with_task_network, populate_with_template_instances, EncodedProblem};
use crate::encoding::Encoding;
use crate::fmt::{format_hddl_plan, format_partial_plan, format_pddl_plan};
use crate::search::{ForwardSearcher, ManualCausalSearch};
use crate::Solver;
use anyhow::Result;
use aries::core::state::Domains;
use aries::core::{IntCst, Lit, VarRef, INT_CST_MAX};
use aries::model::extensions::{AssignmentExt, SavedAssignment};
use aries::model::lang::IAtom;
use aries::model::Model;
use aries::reasoners::stn::theory::{StnConfig, TheoryPropagationLevel};
use aries::solver::parallel::Solution;
use aries::solver::search::activity::*;
use aries::solver::search::conflicts::ConflictBasedBrancher;
use aries::solver::search::lexical::Lexical;
use aries::solver::search::{Brancher, SearchControl};
use aries_planning::chronicles::printer::Printer;
use aries_planning::chronicles::Problem;
use aries_planning::chronicles::*;
use env_param::EnvParam;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

/// If set to true, prints the result of the initial propagation at each depth.
static PRINT_INITIAL_PROPAGATION: EnvParam<bool> = EnvParam::new("ARIES_PRINT_INITIAL_PROPAGATION", "false");

/// If set to true, will print the raw model (before preprocessing)
static PRINT_RAW_MODEL: EnvParam<bool> = EnvParam::new("ARIES_PRINT_RAW_MODEL", "false");

/// If set to true, will print the preprocessed model
static PRINT_MODEL: EnvParam<bool> = EnvParam::new("ARIES_PRINT_MODEL", "false");

pub type SolverResult<Sol> = aries::solver::parallel::SolverResult<Sol>;

#[derive(Copy, Clone, Debug)]
pub enum Metric {
    /// Total duration of the plan
    Makespan,
    /// Number of actions in the plan
    PlanLength,
    /// Sum of all chronicle costs
    ActionCosts,
    /// Minimize value of a given variable
    /// The variable can, e.g., represent the final value of a state variable
    MinimizeVar(IAtom),
    /// Maximize value of a given variable
    /// The variable can, e.g., represent the final value of a state variable
    MaximizeVar(IAtom),
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
    if PRINT_RAW_MODEL.get() {
        Printer::print_problem(&base_problem);
    }
    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(&mut base_problem);
    println!("==========================");
    if PRINT_MODEL.get() {
        Printer::print_problem(&base_problem);
    }

    let metadata = Arc::new(analysis::analyse(&base_problem));

    let mut best_cost = INT_CST_MAX + 1;

    let start = Instant::now();
    for depth in min_depth..=max_depth {
        let mut pb = FiniteProblem {
            model: base_problem.context.model.clone(),
            origin: base_problem.context.origin(),
            horizon: base_problem.context.horizon(),
            makespan_ub: base_problem.context.makespan_ub(),
            chronicles: base_problem.chronicles.clone(),
            meta: metadata.clone(),
        };
        let depth_string = if depth == u32::MAX {
            "∞".to_string()
        } else {
            depth.to_string()
        };
        println!("{depth_string} Solving with depth {depth_string}");
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
        let result = solve_finite_problem(
            pb.clone(),
            strategies,
            metric,
            htn_mode,
            on_new_valid_assignment,
            deadline,
            best_cost - 1,
        );
        println!("  [{:.3}s] Solved", start.elapsed().as_secs_f32());

        let result = result.map(|assignment| (pb, assignment));
        match result {
            SolverResult::Unsat(_) => {} // continue (increase depth)
            SolverResult::Sol((_, (_, cost))) if metric.is_some() && depth < max_depth => {
                let cost = cost.expect("Not cost provided in optimization problem");
                assert!(cost < best_cost);
                best_cost = cost; // continue with new cost bound
            }
            other => return Ok(other.map(|(pb, (ass, _))| (pb, ass))),
        }
    }
    Ok(SolverResult::Unsat(None))
}

/// This function mimics the instantiation of the subproblem, run the propagation and prints the result.
/// and exits immediately.
///
/// Note that is meant to facilitate debugging of the planner during development.
///
/// Returns true if the propagation succeeded.
fn propagate_and_print(pb: &FiniteProblem) -> bool {
    let Ok(EncodedProblem { model, .. }) = encode(pb, None) else {
        println!("==> Invalid model");
        return false;
    };
    let mut solver = init_solver(model);

    println!("\n======== AFTER INITIAL PROPAGATION ======\n");
    let _tmp = if solver.propagate().is_ok() {
        let str = format_partial_plan(pb, &solver.model).unwrap();
        println!("{str}");
        true
    } else {
        println!("==> Propagation failed.");
        false
    };
    std::process::exit(0)
}

pub fn format_plan(problem: &FiniteProblem, assignment: &Domains, htn_mode: bool) -> Result<String> {
    let plan = if htn_mode {
        format!(
            "\n**** Decomposition ****\n\n\
             {}\n\n\
             **** Plan ****\n\n\
             {}",
            format_hddl_plan(problem, assignment)?,
            format_pddl_plan(problem, assignment)?
        )
    } else {
        format_pddl_plan(problem, assignment)?
    };
    Ok(plan)
}

pub fn init_solver(model: Model<VarLabel>) -> Box<Solver> {
    let stn_config = StnConfig {
        theory_propagation: TheoryPropagationLevel::Full,
        ..Default::default()
    };

    let mut solver = Box::new(aries::solver::Solver::new(model));
    solver.reasoners.diff.config = stn_config;
    solver
}

/// Default set of strategies for HTN problems
const HTN_DEFAULT_STRATEGIES: [Strat; 4] = [
    Strat::ActivityBool,
    Strat::ActivityBoolLight,
    Strat::Causal,
    Strat::Forward,
];
/// Default set of strategies for generative (flat) problems.
const GEN_DEFAULT_STRATEGIES: [Strat; 4] = [
    Strat::ActivityBool,
    Strat::ActivityBoolLight,
    Strat::Causal,
    Strat::Forward,
];

#[derive(Copy, Clone, Debug)]
pub enum Strat {
    /// Activity based search
    Activity,
    /// An activity-based variable selection strategy that delays branching on non-boolean variables.
    ActivityBool,
    /// Same as activity-bool with but with a lighter propagation of difference logic constraints.
    ActivityBoolLight,
    /// Mimics forward search in HTN problems.
    Forward,
    /// Search strategy that first tries to solve causal links.
    Causal,
}

/// An activity-based variable selection heuristics that delays branching on temporal variables.
struct ActivityBoolFirstHeuristic;
impl Heuristic<VarLabel> for ActivityBoolFirstHeuristic {
    fn decision_stage(&self, _var: VarRef, label: Option<&VarLabel>, _model: &aries::model::Model<VarLabel>) -> u8 {
        let (lb, ub) = _model.domain_of(_var);
        if ub - lb == 1 {
            return 0;
        }

        match label.as_ref() {
            Some(VarLabel(_, tpe)) => match tpe {
                VarType::Presence | VarType::Reification | VarType::Parameter(_) => 1,
                VarType::ChronicleStart | VarType::ChronicleEnd | VarType::TaskStart(_) | VarType::TaskEnd(_) => 2,
                VarType::Horizon | VarType::Makespan | VarType::EffectEnd | VarType::Cost => 4,
            },
            _ => 3,
        }
    }
}

impl Strat {
    /// Configure the given solver to follow the strategy.
    pub fn adapt_solver(self, solver: &mut Solver, problem: Arc<FiniteProblem>, encoding: Arc<Encoding>) {
        match self {
            Strat::Activity => {
                // nothing, activity based search is the default configuration
            }
            Strat::ActivityBool => {
                solver.set_brancher(ActivityBrancher::new_with_heuristic(ActivityBoolFirstHeuristic))
            }
            Strat::ActivityBoolLight => {
                solver.set_brancher(ActivityBrancher::new_with_heuristic(ActivityBoolFirstHeuristic));
                solver.reasoners.diff.config.theory_propagation = TheoryPropagationLevel::Bounds;
            }
            Strat::Forward => {
                solver.set_brancher(ForwardSearcher::new(problem));
                solver.reasoners.diff.config.theory_propagation = TheoryPropagationLevel::Bounds;
            }
            Strat::Causal => {
                let strat = causal_brancher(problem, encoding);
                solver.set_brancher_boxed(strat);
            }
        }
    }
}

fn causal_brancher(problem: Arc<FiniteProblem>, encoding: Arc<Encoding>) -> Brancher<VarLabel> {
    use aries::solver::search::combinators::CombinatorExt;
    let branching_literals: Vec<Lit> = encoding.tags.iter().map(|&(_, l)| l).collect();

    // manual strategy that lets the user select branches on the command line
    let causal = ManualCausalSearch::new(problem, encoding);

    // conflict directed search on tagged literals only
    let mut conflict = Box::new(ConflictBasedBrancher::new(branching_literals.clone()));
    // when possible, set the value of the prefered value of the branching literal
    for l in branching_literals {
        let var = l.variable();
        if l == Lit::gt(var, 0) {
            conflict.set_default_value(var, 1);
        } else if l == Lit::leq(var, 0) {
            conflict.set_default_value(var, 0);
        }
    }
    // std::process::exit(0);

    // if all tagged literals are set, fallback to standard activity-based search
    let act: Box<ActivityBrancher<VarLabel>> =
        Box::new(ActivityBrancher::new_with_heuristic(ActivityBoolFirstHeuristic));
    let lexical = Box::new(Lexical::with_min());
    let strat = causal.clone_to_box().and_then(conflict).and_then(act).and_then(lexical);

    strat.with_restarts(50, 1.3)
}

impl FromStr for Strat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" | "act" | "activity" => Ok(Strat::Activity),
            "2" | "fwd" | "forward" => Ok(Strat::Forward),
            "3" | "act-bool" | "activity-bool" => Ok(Strat::ActivityBool),
            "4" | "act-bool-light" | "activity-bool-light" => Ok(Strat::ActivityBoolLight),
            "causal" => Ok(Strat::Causal),
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
    pb: Arc<FiniteProblem>,
    strategies: &[Strat],
    metric: Option<Metric>,
    htn_mode: bool,
    on_new_solution: impl Fn(Arc<SavedAssignment>),
    deadline: Option<Instant>,
    cost_upper_bound: IntCst,
) -> SolverResult<(Solution, Option<IntCst>)> {
    if let Some(deadline) = deadline {
        if deadline <= Instant::now() {
            return SolverResult::Timeout(None);
        }
    }
    if PRINT_INITIAL_PROPAGATION.get() {
        propagate_and_print(&pb);
    }
    let Ok(EncodedProblem {
        mut model,
        objective: metric,
        encoding,
    }) = encode(&pb, metric)
    else {
        return SolverResult::Unsat(None);
    };
    if let Some(metric) = metric {
        model.enforce(metric.le_lit(cost_upper_bound), []);
    }
    let solver = init_solver(model);
    let encoding = Arc::new(encoding);

    // select the set of strategies, based on user-input or hard-coded defaults.
    let strats: &[Strat] = if !strategies.is_empty() {
        strategies
    } else if htn_mode {
        &HTN_DEFAULT_STRATEGIES
    } else {
        &GEN_DEFAULT_STRATEGIES
    };
    let mut solver = aries::solver::parallel::ParSolver::new(solver, strats.len(), |id, s| {
        strats[id].adapt_solver(s, pb.clone(), encoding.clone())
    });

    let result = if let Some(metric) = metric {
        solver.minimize_with(metric, on_new_solution, deadline)
    } else {
        solver.solve(deadline)
    };

    // tag result with cost
    let result = result.map(|s| {
        let cost = metric.map(|metric| s.domain_of(metric).0);
        (s, cost)
    });

    if let SolverResult::Sol(_) = result {
        solver.print_stats()
    }
    result
}
