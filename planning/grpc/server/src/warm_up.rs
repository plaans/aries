use anyhow::{bail, Context, Result};
use aries::model::{
    extensions::Shaped,
    lang::{Cst, Rational},
};
use aries_planning::chronicles::{plan::ActionInstance, ChronicleTemplate, Problem, TIME_SCALE};

type Plan = Vec<ActionInstance>;
const SEQUENTIAL_PLAN: &str = "SequentialPlan:";

pub fn plan_from_option(plan: Option<String>, problem: &Problem) -> Result<Option<Plan>> {
    plan.map(|p| plan_from_string(p, problem)).transpose()
}

pub fn plan_from_string(plan: String, problem: &Problem) -> Result<Plan> {
    if plan.starts_with(SEQUENTIAL_PLAN) {
        sequential_plan_from_string(plan, problem)
    } else {
        bail!("Unknown plan format {}", plan.split("\n").next().unwrap());
    }
}

pub fn sequential_plan_from_string(plan: String, problem: &Problem) -> Result<Plan> {
    debug_assert_eq!(plan.split("\n").next(), Some(SEQUENTIAL_PLAN));
    plan.split("\n")
        .skip(1)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(idx, line)| sequential_action_from_string(line, idx, problem))
        .collect()
}

pub fn sequential_action_from_string(line: &str, idx: usize, problem: &Problem) -> Result<ActionInstance> {
    let reduced_line = line.replace(")", "");
    let parts: Vec<&str> = reduced_line.split("(").collect();
    debug_assert_eq!(parts.len(), 2);
    let eps = Rational::new(1, TIME_SCALE.get());

    let name = parts[0].to_string();
    let params: Vec<Cst> = parts[1]
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
        .collect::<Result<_>>()?;
    let start = Rational::from_integer(idx as i32) * eps;
    let duration = eps;

    Ok(ActionInstance {
        name,
        params,
        start,
        duration,
    })
}
