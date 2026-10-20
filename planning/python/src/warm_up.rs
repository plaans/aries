//! Conversion of a unified-planning plan (as a Python object) into a warm-up plan for the solver.
//!
//! This is a port of `planning/grpc/server/src/warm_up.rs`.

use crate::up;
use anyhow::{Context, Result};
use aries_planning::chronicles::{plan::ActionInstance, Problem, TIME_SCALE};
use aries_planning::legacy::*;
use aries_solver::core::IntCst;
use num_rational::Ratio;
use pyo3::prelude::*;

type Plan = Vec<ActionInstance>;

pub fn plan_from_option_upf(plan: Option<&Bound<PyAny>>, problem: &Problem) -> Result<Option<Plan>> {
    plan.map(|p| plan_from_upf(p, problem)).transpose()
}

pub fn plan_from_upf(plan: &Bound<PyAny>, problem: &Problem) -> Result<Plan> {
    // a hierarchical plan is replaced by its flat plan
    let plan = if plan.hasattr("action_plan")? {
        plan.getattr("action_plan")?
    } else {
        plan.clone()
    };
    // `(action instance, start, end)`, with times only for time-triggered plans
    let mut actions = Vec::new();
    if plan.hasattr("timed_actions")? {
        for timed in up::list(&plan.getattr("timed_actions")?)? {
            let (start, action, duration): (Ratio<i64>, Bound<PyAny>, Option<Ratio<i64>>) = timed.extract()?;
            let end = start + duration.unwrap_or_default();
            actions.push((action, Some(start), Some(end)));
        }
    } else {
        for action in up::list(&plan.getattr("actions")?)? {
            actions.push((action, None, None));
        }
    }
    actions
        .iter()
        .enumerate()
        .map(|(idx, (act, start, end))| action_from_upf(act, *start, *end, idx, problem))
        .collect::<Result<Vec<_>>>()
}

pub fn action_from_upf(
    action: &Bound<PyAny>,
    start_time: Option<Ratio<i64>>,
    end_time: Option<Ratio<i64>>,
    idx: usize,
    problem: &Problem,
) -> Result<ActionInstance> {
    let name = up::name(&action.getattr("action")?)?;

    let params = up::list(&action.getattr("actual_parameters")?)?
        .iter()
        .map(|p| {
            param_from_upf(
                &up::read_expression(p)?.atom.context("Parameter is not an atom")?,
                problem,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let eps = Rational::new(1, TIME_SCALE.get());
    let start = start_time
        .map(real_to_rational)
        .unwrap_or_else(|| Ok(Rational::from_integer(idx as IntCst) * eps))?;
    let end = end_time
        .map(real_to_rational)
        .unwrap_or_else(|| Ok(Rational::from_integer(idx as IntCst) * eps))?;
    let duration = end - start;

    Ok(ActionInstance {
        name,
        params,
        start,
        duration,
    })
}

pub fn param_from_upf(param: &up::Atom, problem: &Problem) -> Result<Cst> {
    let content = param.content.clone().with_context(|| "Parameter content is empty")?;
    match content {
        up::atom::Content::Symbol(s) => {
            let sym = problem
                .context
                .get_symbol_table()
                .id(&s)
                .with_context(|| format!("Unknown parameter {}", s))?;
            Ok(problem.context.typed_sym(sym).into())
        }
        up::atom::Content::Int(i) => {
            let val: IntCst = i.try_into().with_context(|| format!("Invalid int value {}", i))?;
            Ok(Cst::Int(val))
        }
        up::atom::Content::Real(r) => {
            let val = real_to_rational(Ratio::new(r.numerator, r.denominator))?;
            Ok(Cst::Fixed(val))
        }
        up::atom::Content::Boolean(b) => Ok(Cst::Bool(b)),
    }
}

pub fn real_to_rational(r: Ratio<i64>) -> Result<Rational> {
    let num: IntCst = (*r.numer())
        .try_into()
        .with_context(|| format!("Invalid numerator {}", r.numer()))?;
    let denom: IntCst = (*r.denom())
        .try_into()
        .with_context(|| format!("Invalid denominator {}", r.denom()))?;
    Ok(Rational::new(num, denom))
}
