use crate::models::{
    env::Env,
    time::{TemporalInterval, Timepoint},
};

/// Represents a struct which has a duration.
pub trait Durative<E> {
    fn start(&self, env: &Env<E>) -> &Timepoint;
    fn end(&self, env: &Env<E>) -> &Timepoint;
    fn is_start_open(&self) -> bool;
    fn is_end_open(&self) -> bool;
    fn into_temporal_interval(&self, env: &Env<E>) -> TemporalInterval {
        TemporalInterval::new(
            self.start(env).clone(),
            self.end(env).clone(),
            self.is_start_open(),
            self.is_end_open(),
        )
    }
}
