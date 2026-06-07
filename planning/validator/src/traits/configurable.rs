use crate::models::{env::Env, parameter::Parameter};

use super::suffix_params::SuffixParams;

/// Represents a struct that can extend the environment with parameters.
pub trait Configurable<E: Clone>: SuffixParams {
    fn id(&self) -> &str;
    fn params(&self) -> &[Parameter];
    fn new_env_with_params(&self, env: &Env<E>) -> Env<E> {
        let mut new_env = env.clone();
        for param in self.params().iter() {
            param.bound(&mut new_env);
        }
        new_env
    }
    fn suffix_params_with_id(&mut self) -> anyhow::Result<()> {
        let id = self.id().to_string();
        self.suffix_params_with(&id)
    }
}
