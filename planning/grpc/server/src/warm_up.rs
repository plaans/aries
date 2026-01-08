use anyhow::{Context, Result};
use aries::{
    core::IntCst,
    model::{
        extensions::Shaped,
        lang::{Cst, Rational},
    },
};
use aries_planning::chronicles::{plan::ActionInstance, Problem, TIME_SCALE};
use unified_planning as up;

type Plan = Vec<ActionInstance>;

pub fn plan_from_option_upf(plan: Option<up::Plan>, problem: &Problem) -> Result<Option<Plan>> {
    plan.map(|p| plan_from_upf(p, problem)).transpose()
}

pub fn plan_from_upf(plan: up::Plan, problem: &Problem) -> Result<Plan> {
    plan.actions
        .iter()
        .enumerate()
        .map(|(idx, act)| action_from_upf(act, idx, problem))
        .collect::<Result<Vec<_>>>()
}

pub fn action_from_upf(action: &up::ActionInstance, idx: usize, problem: &Problem) -> Result<ActionInstance> {
    let name = action.action_name.clone();

    let params = action
        .parameters
        .iter()
        .map(|p| param_from_upf(p, problem))
        .collect::<Result<Vec<_>>>()?;

    let eps = Rational::new(1, TIME_SCALE.get());
    let start = action
        .start_time
        .map(real_to_rational)
        .unwrap_or_else(|| Ok(Rational::from_integer(idx as IntCst) * eps))?;
    let end = action
        .end_time
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
                .model
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
            let val = real_to_rational(r)?;
            Ok(Cst::Fixed(val))
        }
        up::atom::Content::Boolean(b) => Ok(Cst::Bool(b)),
    }
}

pub fn real_to_rational(r: up::Real) -> Result<Rational> {
    let num: IntCst = r
        .numerator
        .try_into()
        .with_context(|| format!("Invalid numerator {}", r.numerator))?;
    let denom: IntCst = r
        .denominator
        .try_into()
        .with_context(|| format!("Invalid denominator {}", r.denominator))?;
    Ok(Rational::new(num, denom))
}
