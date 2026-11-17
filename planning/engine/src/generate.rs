use std::{
    collections::BTreeMap,
    fmt::Display,
    path::PathBuf,
    time::{Duration, Instant},
};

use aries_bench_data::SolveStatus;
use aries_plan_engine::encode::{encoding::Encoding, tags::Tag};
use aries_solver::{core::state::Evaluable, prelude::*, solver::stats::Stats};
use planx::{Model, Res, SolutionPlan};
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

pub struct SolveFiniteArgs {
    pub optimize: bool,
    pub max_instances: u32,
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

    let num_free_instances_per_action =
        BTreeMap::from_iter(model.actions.iter().map(|a| (a.name.clone(), options.max_instances)));

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
