use anyhow::Result;

use super::{action::ValAction, env::Env};

/// The minimal behaviour of a problem to validate a plan.
pub trait ValProblem {
    /// Creates the initial environment for the validation.
    ///
    /// Notes that the initial state is stored in this environment.
    fn initial_env(&self, verbose: bool) -> Result<Env>;

    /// Returns the action with the given name.
    fn get_action(&self, name: String) -> Result<Box<dyn ValAction>>;
}
