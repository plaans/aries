use aries_plan_engine::plans::lifted_plan::LiftedPlan;
use planx::{Model, Res};

use crate::optimize_plan::{self, encode_plan_optimization_problem};

#[derive(clap::Args, Debug, Clone)]
pub struct Options {}

pub fn validate(model: &Model, plan: &LiftedPlan, _options: &Options) -> Res<bool> {
    // we frame the problem as an optimization problem with no relaxation,
    // hence the solver is forced to reproduce the plan
    let opt_options = crate::optimize_plan::Options {
        relaxation: vec![],                              // no relaxation
        objective: optimize_plan::Objective::PlanLength, // TODO: change to domain's metric
    };
    let mut solver = encode_plan_optimization_problem(model, plan, &opt_options)?;

    Ok(solver.check_satisfiability())
}
