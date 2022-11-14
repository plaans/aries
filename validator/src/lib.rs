pub mod interfaces;
mod macros;
mod models;
mod procedures;
mod traits;

use std::fmt::Debug;

use anyhow::{bail, Result};
use models::{action::Action, condition::Condition, env::Env};
use traits::interpreter::Interpreter;

use crate::traits::act::Act;

pub fn validate<'a, E: Interpreter + Debug + 'a>(
    env: &mut Env<E>,
    actions: impl Iterator<Item = &'a Action<E>>,
    goals: impl Iterator<Item = &'a Condition<E>>,
) -> Result<()> {
    print_info!(env.verbose, "Simulation of the plan");
    for a in actions {
        let new_env = env.extends_with(a.local_env());
        if let Some(s) = a.apply(&new_env, env.state())? {
            env.set_state(s);
        } else {
            bail!("Non applicable action {:?}", a);
        }
    }

    print_info!(env.verbose, "Check the goal has been reached");
    for g in goals {
        if !g.is_valid(env)? {
            bail!("Unreached goal {:?}", g);
        }
    }

    print_info!(env.verbose, "The plan is valid");
    Ok(())
}
