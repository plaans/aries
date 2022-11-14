use std::convert::{TryFrom, TryInto};

use anyhow::Result;
use unified_planning::{Expression, Plan, Problem};

use crate::{
    models::{action::ActionIter, env::Env, goal::GoalIter},
    print_info, validate,
};

mod constants;
mod expression;
mod factories;
mod plan;
mod problem;
mod utils;

pub fn validate_upf(problem: &Problem, plan: &Plan, verbose: bool) -> Result<()> {
    print_info!(verbose, "Creation of the initial state");
    let mut env: Env<Expression> = problem.clone().try_into()?;
    env.verbose = verbose;

    print_info!(verbose, "Creation of the actions and the goals");
    let actions = ActionIter::try_from((problem.clone(), plan.clone()))?;
    let goals = GoalIter::try_from(problem.clone())?;

    print_info!(verbose, "Start the validation");
    validate(&mut env, actions.iter(), goals.iter())
}
