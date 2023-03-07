use crate::models::time::Timepoint;

/// Represents a struct which has a duration.
pub trait Durative<E> {
    fn start(&self) -> &Timepoint;
    fn end(&self) -> &Timepoint;
    fn is_start_open(&self) -> bool;
    fn is_end_open(&self) -> bool;
}
