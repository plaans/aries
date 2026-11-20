use std::{
    collections::BTreeMap,
    fmt::Display,
    path::PathBuf,
    time::{Duration, Instant},
};

use aries_bench_data::SolveStatus;
use aries_plan_engine::encode::{encoding::Encoding, tags::Tag};
use aries_solver::{core::state::Evaluable, prelude::*, solver::stats::Stats};
use planx::{
    ActionRef, Model, Res, SolutionPlan,
    preprocessing::analysis::{CountConstraints, PlanScope},
};
use timelines::{
    Sched,
    explain::{ExplainableSolver, OptimizeResult},
};

use crate::{
    export::{self},
    optimize_plan::{self, Objective},
};

pub type RelaxableConstraint = Tag;

#[derive(clap::Args, Debug, Clone)]
pub struct SolveOptions {
    /// Defines the maximum number of instances per action template.
    ///
    /// For example, if set to 3, the resulting plan may have *at most* three instances
    /// of a `pick` action and at most 3 instances of a `drop` action.
    #[arg(short, long)]
    pub max_depth: Option<u32>,

    /// First depth of deepening search
    #[arg(long)]
    pub min_depth: Option<u32>,

    /// Bounds on the number of instances of each action template, derived by the analysis of the model
    /// (see `ape analyze`), and the plans for which they hold.
    /// At depth `d`, the number of instances of a template with bounds `[lb, ub]` is `min(lb + d, ub)`.
    /// Without bounds (`none`), it is `d`.
    ///
    /// `nr` and `nr+` only hold for non-redundant plans: they preserve the existence of a plan (any plan can be
    /// made non-redundant by removing actions) but may exclude the best plans for some metrics.
    #[arg(long, value_enum, default_value_t = CountBounds::NrPlus)]
    pub count_bounds: CountBounds,

    /// Maximum allowed runtime, in seconds.
    #[arg(short = 't', long = "timeout")]
    pub timeout_s: Option<f64>,

    /// Defines the objective to be minimized (overriding the ones in the problem)
    #[arg(short, long, num_args(0..))]
    pub objectives: Vec<Objective>,

    /// If provided, the final plan will be written to this file.
    #[arg(short = 'w', long = "write-plan")]
    pub plan_file: Option<PathBuf>,

    /// If provided, the benchmark report will be written to a file in this directory.
    #[arg(short = 'r', long = "write-report")]
    pub report_dir: Option<PathBuf>,
}
#[derive(clap::Args, Debug, Clone)]
pub struct SolveFiniteOptions {
    /// Defines the maximum number of instances per action template.
    ///
    /// For instance, if set to 3, the resulting plan may have *at most* three instances
    /// of a `pick` action and at most 3 instances of a `drop` action.
    #[arg(short, long)]
    pub max_instances: u32,

    /// Maximum allowed runtime, in seconds.
    #[arg(short = 't', long = "timeout")]
    pub timeout_s: Option<f64>,

    /// Defines the objective to be minimized
    #[arg(short, long, num_args(0..))]
    pub objectives: Vec<Objective>,

    /// If set, the planner will try to find the optimal solution
    #[arg(long)]
    pub optimize: bool,

    /// If provided, the final plan will be written to this file.
    #[arg(short = 'w', long = "write-plan")]
    pub plan_file: Option<PathBuf>,

    /// If provided, the benchmark report will be written to a file in this directory.
    #[arg(short = 'r', long = "write-report")]
    pub report_dir: Option<PathBuf>,
}

/// Which bounds on the number of instances of action templates are used.
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountBounds {
    /// No bounds.
    None,
    /// Bounds holding for all plans.
    All,
    /// Bounds holding for non-redundant plans.
    Nr,
    /// Bounds holding for non-redundant plans without actions cancelling each other.
    #[value(name = "nr+")]
    NrPlus,
}

impl std::fmt::Display for CountBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CountBounds::None => write!(f, "none"),
            CountBounds::All => write!(f, "all"),
            CountBounds::Nr => write!(f, "nr"),
            CountBounds::NrPlus => write!(f, "nr+"),
        }
    }
}

/// Number of instances of each action template allowed at each depth of the deepening search.
pub struct InstanceLimits {
    /// Lower and upper bound on the number of instances of each template.
    bounds: BTreeMap<ActionRef, (u32, Option<u32>)>,
}

impl InstanceLimits {
    /// No bounds: `d` instances of each template at depth `d`.
    pub fn unbounded(model: &Model) -> Self {
        let bounds = model.actions.iter().map(|a| (a.name.clone(), (0, None))).collect();
        InstanceLimits { bounds }
    }

    /// Bounds derived from the count constraints of an annotated model (see `ape analyze`).
    /// Returns `None` if the constraints show that no plan exists (in the selected scope).
    pub fn from_constraints(model: &Model, which: CountBounds) -> Option<Self> {
        let scope = match which {
            CountBounds::None => return Some(Self::unbounded(model)),
            CountBounds::All => PlanScope::AllPlans,
            CountBounds::Nr => PlanScope::NonRedundant,
            CountBounds::NrPlus => PlanScope::NonRedundantPlus,
        };
        let propagated = CountConstraints::new(model).propagate(scope)?;
        let clamp = |x: i64| x.clamp(0, u32::MAX as i64) as u32;
        let bounds = (propagated.into_iter())
            .map(|(a, (lb, ub))| (a, (clamp(lb), ub.map(clamp))))
            .collect();
        Some(InstanceLimits { bounds })
    }

    /// Number of instances of each template at the given depth: `min(lb + depth, ub)`.
    pub fn at_depth(&self, depth: u32) -> BTreeMap<ActionRef, u32> {
        (self.bounds.iter())
            .map(|(a, &(lb, ub))| {
                let n = lb.saturating_add(depth);
                (a.clone(), ub.map_or(n, |ub| n.min(ub)))
            })
            .collect()
    }

    /// The smallest depth at which all templates reach their upper bound, if they are all finite.
    /// Deeper subproblems are identical to this one.
    pub fn max_depth(&self) -> Option<u32> {
        (self.bounds.values())
            .map(|&(lb, ub)| ub.map(|ub| ub.saturating_sub(lb)))
            .try_fold(0, |acc, d| d.map(|d| acc.max(d)))
    }
}

impl Display for InstanceLimits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let width = self.bounds.keys().map(|a| a.to_string().len()).max().unwrap_or(0);
        for (a, (lb, ub)) in &self.bounds {
            let ub = ub.map_or("inf".to_string(), |ub| ub.to_string());
            writeln!(f, "  {:width$}  [{lb}, {ub}]", a.to_string())?;
        }
        Ok(())
    }
}

pub struct SolveFiniteArgs {
    pub optimize: bool,
    /// Maximum number of instances of each action template.
    pub max_instances: BTreeMap<ActionRef, u32>,
    pub plan_file: Option<PathBuf>,
    pub deadline: Option<Instant>,
}

pub struct PlanGenerationResult {
    pub solution: Option<SolutionPlan>,
    pub status: export::SolveStatus,
    pub runtime: std::time::Duration,
    pub solver_stats: Stats,
}

impl PlanGenerationResult {
    pub fn trivial_unsat() -> Self {
        Self {
            solution: None,
            status: export::SolveStatus::SolvedUnsat,
            runtime: Duration::ZERO,
            solver_stats: Stats::new(),
        }
    }
    pub fn trivial_timeout() -> Self {
        Self {
            solution: None,
            status: export::SolveStatus::Timeout,
            runtime: Duration::ZERO,
            solver_stats: Stats::new(),
        }
    }

    /// Merge two results over two subproblems.
    pub fn merge_split_solve(&mut self, next: PlanGenerationResult) {
        if next.solution.is_some() {
            // assumes a new solution is always better (we can't check without knowing wether it is a maximization or minimzation problem)
            self.solution = next.solution;
        }
        use aries_bench_data::SolveStatus::*;
        self.status = match (self.status, next.status) {
            (Timeout, SolvedUnsat) => Timeout,
            (SolvedUnsat, other) | (other, SolvedUnsat) => other,
            (SolvedSat, _) | (_, SolvedSat) => SolvedSat, // we have a solution but loose any optimality guarantee
            (Timeout, SolvedOpt) | (SolvedOpt, Timeout) => SolvedSat,
            (SolvedOpt, SolvedOpt) => SolvedOpt,
            (Timeout, Timeout) => Timeout,
        };
        self.runtime += next.runtime;
        self.solver_stats += next.solver_stats;
    }
}

impl Display for PlanGenerationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "; status: {:?}", self.status)?;
        writeln!(f, "; runtime: {:.3}s", self.runtime.as_secs_f64())?;
        writeln!(f, "; num-conflicts: {}", self.solver_stats.num_conflicts)?;
        writeln!(f, "; num-decisions: {}", self.solver_stats.num_decisions)?;
        if let Some(plan) = self.solution.as_ref() {
            write!(f, "{plan}")?;
        }
        Ok(())
    }
}

pub fn solve_finite_planning_problem(model: &Model, options: &SolveFiniteArgs) -> Res<PlanGenerationResult> {
    // create a dummy plan with the appropriate number of actions
    // this is temporary a workaround to reuse the existing `optimize_plan` facilities
    let plan = SolutionPlan::empty();

    let start = Instant::now();
    let (mut solver, encoding, _sched) = encode_finite_planning_problem(model, &plan, options)?;

    let _encoding_time = start.elapsed().as_millis();

    let objective = encoding
        .objectives
        .first()
        .copied()
        .expect("no objective specified (no default)");

    // set the objective to a constant if we are not optimizing
    let solver_objective = if options.optimize { objective } else { 0.into() };

    let print = |sol: &Solution| {
        println!("\n==== Plan (objective: {}) =====", objective.evaluate(sol).unwrap());
        println!("{}\n", encoding.plan(sol).export(model).unwrap());
    };

    let deadline = options.deadline.map(SearchLimit::Deadline).unwrap_or(SearchLimit::None);
    let result = solver.optimize(solver_objective, &print, [], deadline);

    let plan = if let Some(solution) = result.solution() {
        println!(
            "\n> Found {}solution:",
            if result.has_optimal() && options.optimize {
                "optimal "
            } else {
                ""
            }
        );
        print(solution);
        let plan = encoding.plan(solution).export(model)?;
        plan.write_to_file(options.plan_file.as_ref())?;
        Some(plan)
    } else {
        println!("No solution !!!!");
        None
    };

    let res = PlanGenerationResult {
        runtime: start.elapsed(),
        status: match result {
            OptimizeResult::Opt(_sol) if options.optimize => SolveStatus::SolvedOpt,
            OptimizeResult::Opt(_sol) => SolveStatus::SolvedSat,
            OptimizeResult::Unsat => SolveStatus::SolvedUnsat,
            OptimizeResult::Timeout(Some(_)) => SolveStatus::SolvedSat,
            OptimizeResult::Timeout(None) => SolveStatus::Timeout,
        },
        solution: plan,
        solver_stats: solver.get().stats.clone(),
    };

    Ok(res)
}

fn encode_finite_planning_problem(
    model: &Model,
    plan: &SolutionPlan,
    options: &SolveFiniteArgs,
) -> Res<(ExplainableSolver<RelaxableConstraint>, Encoding, Sched)> {
    // TODO: make specific function.
    // - ability to specify explanations vocabulary via RelaxableConstraint (Tag), including removing (pre)conditions (like in domain repair).

    let num_free_instances_per_action = options.max_instances.clone();

    optimize_plan::encode_plan_optimization_problem(
        model,
        plan,
        num_free_instances_per_action,
        &optimize_plan::Options {
            relaxation: vec![
                optimize_plan::Relaxation::ActionPresence,
                optimize_plan::Relaxation::StartTime,
            ],
            objectives: vec![], // unused, TODO: remove from parameters
        },
        false,
    )
}
