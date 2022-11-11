mod interfaces;
mod macros;
mod models;
mod procedures;
mod traits;

use std::convert::{TryFrom, TryInto};

use anyhow::{bail, Result};
use models::{action::ActionIter, env::Env};
use traits::interpreter::Interpreter;

use crate::traits::act::Act;

pub fn validate<E, Pb, Pl>(problem: &Pb, plan: &Pl, verbose: bool) -> Result<()>
where
    E: Interpreter + std::fmt::Debug,
    Pb: Clone + TryInto<Env<E>, Error = anyhow::Error>,
    Pl: Clone,
    ActionIter<E>: TryFrom<(Pb, Pl), Error = anyhow::Error>,
{
    print_info!(verbose, "Creation of the initial state");
    let mut env: Env<E> = problem.clone().try_into()?;
    env.verbose = verbose;

    print_info!(verbose, "Simulation of the plan");
    let actions = ActionIter::try_from((problem.clone(), plan.clone()))?;
    for a in actions.iter() {
        let new_env = env.extends_with(a.local_env());
        if let Some(s) = a.apply(&new_env, env.state())? {
            env.set_state(s);
        } else {
            bail!("Non applicable action {:?}", a);
        }
    }

    print_info!(verbose, "Check the goal has been reached");
    // TODO
    Ok(())
}
