use anyhow::{Context, Result};
use aries::{
    core::IntCst,
    model::{
        extensions::Shaped,
        lang::{Cst, Rational},
    },
};
use aries_planning::chronicles::{plan::ActionInstance, Problem, TIME_SCALE};
use regex::Regex;

type Plan = Vec<ActionInstance>;
const SUPPORTED_PLAN_FORMATS: [&str; 2] = ["SequentialPlan:", "TimeTriggeredPlan:"];

pub fn plan_from_option(plan: Option<String>, problem: &Problem) -> Result<Option<Plan>> {
    plan.map(|p| plan_from_string(p, problem)).transpose()
}

pub fn plan_from_string(plan: String, problem: &Problem) -> Result<Plan> {
    let plan_format = plan.split("\n").next().unwrap();
    debug_assert!(SUPPORTED_PLAN_FORMATS.contains(&plan_format));

    plan.split("\n")
        .skip(1) // Skip the first line with the plan format
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(idx, line)| action_from_string(line, idx, problem))
        .collect()
}

pub fn action_from_string(line: &str, idx: usize, problem: &Problem) -> Result<ActionInstance> {
    let eps = Rational::new(1, TIME_SCALE.get());
    let regex = Regex::new(
        r"(?m)^\s*(?:(?<start>\d+\.\d+):\s?)?(?<name>[\w-]+)(?:\((?<params>[\w, -]+)\))?\s?(?:\[(?<duration>\d+\.\d+)\])?",
    )?;
    let captures = regex
        .captures(line)
        .with_context(|| format!("Invalid action line: {}", line))?;

    let name = captures.name("name").unwrap().as_str().trim().to_string();

    let params = captures
        .name("params")
        .map(|m| m.as_str())
        .map(|params| params_from_string(params, problem))
        .unwrap_or_else(|| Ok(vec![]))?;

    let time_from_capture = |name: &str| {
        captures
            .name(name)
            .map(|m| m.as_str())
            .map(time_from_string)
            .transpose()
    };
    let start = time_from_capture("start")?.unwrap_or_else(|| Rational::from_integer(idx as IntCst) * eps);
    let duration = time_from_capture("duration")?.unwrap_or_else(|| Rational::from_integer(0));

    Ok(ActionInstance {
        name,
        params,
        start,
        duration,
    })
}

pub fn params_from_string(params: &str, problem: &Problem) -> Result<Vec<Cst>> {
    params
        .split(",")
        .map(|p| p.trim())
        .map(|p| {
            let sym = problem
                .context
                .model
                .get_symbol_table()
                .id(&p.to_string())
                .with_context(|| format!("Unknown parameter {}", p))?;
            Ok(problem.context.typed_sym(sym).into())
        })
        .collect()
}

pub fn time_from_string(time: &str) -> Result<Rational> {
    if !time.contains(".") {
        Ok(Rational::from_integer(time.parse::<IntCst>()?))
    } else {
        let parts: Vec<_> = time.split(".").collect();
        debug_assert_eq!(parts.len(), 2);
        let denom = (10 as IntCst).pow(parts[1].len() as u32);
        let numer = parts[0].parse::<IntCst>()? * denom + parts[1].parse::<IntCst>()?;
        Ok(Rational::new(numer, denom))
    }
}
