use crate::models::{env::Env, time::Timepoint};

/// Represents a struct which has a duration.
pub trait Durative<E> {
    fn start(&self, env: &Env<E>) -> &Timepoint;
    fn end(&self, env: &Env<E>) -> &Timepoint;
    fn is_start_open(&self) -> bool;
    fn is_end_open(&self) -> bool;
}
