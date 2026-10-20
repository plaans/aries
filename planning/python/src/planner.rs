//! Resolution of a problem with the Aries planner.
//!
//! This is a port of `solve_blocking` in `planning/grpc/server/src/bin/server.rs`.

use crate::serialize::{serialize_plan, Plan, ProblemInfo};
use anyhow::{ensure, Context, Error};
use aries_planners::solver;
use aries_planners::solver::{Metric, SolverResult, Strat};
use aries_planning::chronicles::FiniteProblem;
use aries_solver::prelude::*;
use itertools::Itertools;
use pyo3::IntoPyObject;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct SolverConfiguration {
    /// If true, the solver should look for optimal solutions
    pub optimal: bool,
    /// Timeout (s) after which search will stop
    pub timeout: Option<f64>,
    /// Minimal depth for the search.
    pub min_depth: u32,
    /// Maximal depth for the search.
    pub max_depth: u32,
    /// If not empty, the solver will only run the specified strategies instead of default set of strategies.
    pub strategies: Vec<Strat>,
}

/// Result of a resolution, converted to a `PlanGenerationResult` on the Python side.
#[derive(Debug, IntoPyObject)]
pub struct PlanGenerationResult {
    /// Name of the corresponding `PlanGenerationResultStatus`
    pub status: &'static str,
    pub plan: Option<Plan>,
    pub metrics: HashMap<String, String>,
    pub log_messages: Vec<String>,
}

impl PlanGenerationResult {
    pub fn new(status: &'static str, plan: Option<Plan>) -> Self {
        PlanGenerationResult {
            status,
            plan,
            metrics: Default::default(),
            log_messages: vec![],
        }
    }

    pub fn internal_error(e: &Error) -> Self {
        let mut result = PlanGenerationResult::new("INTERNAL_ERROR", None);
        result
            .log_messages
            .push(format!("{}", e.chain().rev().format("\n    Context: ")));
        result
    }

    /// Adds a measure of the time spent in the engine in the metrics
    pub fn with_engine_time(mut self, start: &Instant) -> Self {
        self.metrics.insert(
            "engine_internal_time".to_string(),
            format!("{:.6}", start.elapsed().as_secs_f64()),
        );
        self
    }
}

/// Problem converted to chronicles, together with all information needed to solve it.
pub struct Request {
    pub info: ProblemInfo,
    pub base_problem: aries_planning::chronicles::Problem,
    pub htn_mode: bool,
    pub bounded: bool,
    pub metric: Option<Metric>,
    pub warm_up_plan: Option<Vec<aries_planning::chronicles::plan::ActionInstance>>,
}

/// Solves the given problem, giving any intermediate solution to the callback.
/// NOTE: This function is CPU-Bound and does not need the Python GIL.
pub fn solve_blocking(
    request: Request,
    on_new_sol: impl Fn(Plan) + Clone,
    conf: &SolverConfiguration,
    reception_time: Instant,
) -> Result<PlanGenerationResult, Error> {
    let Request {
        info,
        base_problem,
        htn_mode,
        bounded,
        metric,
        warm_up_plan,
    } = request;
    let deadline = conf
        .timeout
        .map(|timeout| reception_time + std::time::Duration::from_secs_f64(timeout));

    let max_depth = conf.max_depth;
    let min_depth = if bounded {
        max_depth // non recursive htn: bounded size, go directly to max
    } else {
        conf.min_depth
    };

    // callback that will be invoked each time an intermediate solution is found
    let on_new_solution = |pb: &FiniteProblem, ass: Solution| {
        let plan = serialize_plan(&info, pb, &ass);
        match plan {
            Ok(plan) => on_new_sol(plan),
            Err(err) => eprintln!("Error when serializing intermediate plan: {err}"),
        }
    };
    // run solver
    let result = solver::solve(
        base_problem,
        min_depth,
        max_depth,
        &conf.strategies,
        metric,
        htn_mode,
        warm_up_plan,
        on_new_solution,
        deadline,
    )?;
    match result {
        SolverResult::Sol((finite_problem, plan)) => {
            println!(
                "************* SOLUTION FOUND **************\n\n{}",
                solver::format_plan(&finite_problem, &plan, htn_mode)?
            );
            let status = if metric.is_some() && bounded {
                "SOLVED_OPTIMALLY"
            } else {
                "SOLVED_SATISFICING"
            };
            let plan = serialize_plan(&info, &finite_problem, &plan)?;
            Ok(PlanGenerationResult::new(status, Some(plan)))
        }
        SolverResult::Unsat(_) => {
            println!("************* NO PLAN **************");
            Ok(PlanGenerationResult::new("UNSOLVABLE_INCOMPLETELY", None))
        }
        SolverResult::Timeout(opt_plan) => {
            println!("************* TIMEOUT **************");
            let opt_plan = if let Some((finite_problem, plan)) = opt_plan {
                println!("\n{}", solver::format_plan(&finite_problem, &plan, htn_mode)?);
                Some(serialize_plan(&info, &finite_problem, &plan)?)
            } else {
                None
            };

            let status = if opt_plan.is_none() || conf.optimal {
                "TIMEOUT"
            } else {
                "SOLVED_SATISFICING"
            };

            Ok(PlanGenerationResult::new(status, opt_plan))
        }
    }
}

/// Selects the metric to optimize, if any.
pub fn read_metric(
    metrics: &[crate::up::Metric],
    base_problem: &aries_planning::chronicles::Problem,
    conf: &SolverConfiguration,
) -> Result<Option<Metric>, Error> {
    use crate::up::MetricKind;
    ensure!(metrics.len() <= 1, "Unsupported: multiple metrics provided.");
    let metric = if !conf.optimal {
        None
    } else if let Some(metric) = metrics.first() {
        match metric.kind {
            MetricKind::MinimizeActionCosts => Some(Metric::ActionCosts),
            MetricKind::MinimizeSequentialPlanLength => Some(Metric::PlanLength),
            MetricKind::MinimizeMakespan => Some(Metric::Makespan),
            MetricKind::MinimizeExpressionOnFinalState => Some(Metric::MinimizeVar(
                base_problem
                    .context
                    .metric_final_value()
                    .context("Trying to minimize an empty expression metric.")?,
            )),
            MetricKind::MaximizeExpressionOnFinalState => Some(Metric::MaximizeVar(
                base_problem
                    .context
                    .metric_final_value()
                    .context("Trying to maximize an empty expression metric.")?,
            )),
            kind => anyhow::bail!("Unsupported metric kind: {kind:?}"),
        }
    } else {
        None
    };
    Ok(metric)
}
