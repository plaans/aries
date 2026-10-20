//! Python bindings of the Aries planner for the unified-planning library.
//!
//! The unified-planning engines are defined in the `up_aries_pyo3` Python package that wraps this module.

mod convert;
mod planner;
mod serialize;
mod up;
mod warm_up;

use crate::planner::{PlanGenerationResult, Request, SolverConfiguration};
use crate::serialize::ProblemInfo;
use anyhow::Context;
use aries_planners::solver::Strat;
use aries_planning::chronicles::analysis::hierarchy::hierarchical_is_non_recursive;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::time::Instant;

/// Converts the problem and warm-up plan, which requires the GIL.
fn build_request(
    problem: &Bound<PyAny>,
    warm_start_plan: Option<&Bound<PyAny>>,
    conf: &SolverConfiguration,
) -> anyhow::Result<Request> {
    let problem_name: String = problem.getattr("name")?.str()?.extract()?;
    let base_problem = convert::problem_to_chronicles(problem).with_context(|| format!("In problem {problem_name}"))?;
    let htn_mode = problem.hasattr("task_network")?;
    let bounded = htn_mode && hierarchical_is_non_recursive(&base_problem) || base_problem.templates.is_empty();
    let metric = planner::read_metric(&up::read_metrics(problem)?, &base_problem, conf)?;
    let warm_up_plan = warm_up::plan_from_option_upf(warm_start_plan, &base_problem)?;

    let activities = if problem.hasattr("activities")? {
        let mut activities = HashMap::new();
        for activity in up::list(&problem.getattr("activities")?)? {
            let parameters = up::list(&activity.getattr("parameters")?)?
                .iter()
                .map(up::name)
                .collect::<anyhow::Result<Vec<_>>>()?;
            activities.insert(up::name(&activity)?, parameters);
        }
        Some(activities)
    } else {
        None
    };
    let info = ProblemInfo {
        hierarchical: htn_mode,
        temporal: up::has_feature(problem, "CONTINUOUS_TIME")? || up::has_feature(problem, "DISCRETE_TIME")?,
        activities,
    };
    Ok(Request {
        info,
        base_problem,
        htn_mode,
        bounded,
        metric,
        warm_up_plan,
    })
}

/// Solves a unified-planning problem.
///
/// Returns a dict with the fields of a `PlanGenerationResult`, where the status is given by its name
/// and the plan as plain data. Each intermediate plan is passed to `on_solution` (if any) as soon as it is found.
/// The GIL is released while solving.
#[pyfunction]
#[pyo3(signature = (problem, optimal=false, timeout=None, min_depth=0, max_depth=u32::MAX, strategies=vec![], warm_start_plan=None, on_solution=None))]
#[allow(clippy::too_many_arguments)]
fn solve(
    py: Python<'_>,
    problem: &Bound<PyAny>,
    optimal: bool,
    timeout: Option<f64>,
    min_depth: u32,
    max_depth: u32,
    strategies: Vec<String>,
    warm_start_plan: Option<&Bound<PyAny>>,
    on_solution: Option<Py<PyAny>>,
) -> PyResult<PlanGenerationResult> {
    let reception_time = Instant::now();
    let strategies = strategies
        .iter()
        .map(|s| s.parse::<Strat>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    let conf = SolverConfiguration {
        optimal,
        timeout,
        min_depth,
        max_depth,
        strategies,
    };

    let request = match build_request(problem, warm_start_plan, &conf) {
        Ok(request) => request,
        Err(e) => return Ok(PlanGenerationResult::internal_error(&e).with_engine_time(&reception_time)),
    };

    let on_new_sol = |plan: serialize::Plan| {
        let Some(callback) = on_solution.as_ref() else {
            return;
        };
        let result = PlanGenerationResult::new("INTERMEDIATE", Some(plan)).with_engine_time(&reception_time);
        Python::attach(|py| {
            if let Err(e) = callback.call1(py, (result,)) {
                e.print(py);
            }
        });
    };

    let result = py.detach(|| planner::solve_blocking(request, on_new_sol, &conf, reception_time));
    let result = result.unwrap_or_else(|e| PlanGenerationResult::internal_error(&e));
    Ok(result.with_engine_time(&reception_time))
}

#[pymodule]
fn _aries(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(solve, m)?)?;
    Ok(())
}
