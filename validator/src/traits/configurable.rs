use crate::models::{env::Env, parameter::Parameter};

/// Represents a struct that can extend the environment with parameters.
pub trait Configurable<E: Clone> {
    fn params(&self) -> &[Parameter];
    fn new_env_with_params(&self, env: &Env<E>) -> Env<E> {
        let mut new_env = env.clone();
        for param in self.params().iter() {
            param.bound(&mut new_env);
        }
        new_env
    }
}
