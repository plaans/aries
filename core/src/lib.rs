use crate::state::Cause;
pub use bound_value::*;
pub use lit::*;
pub use var_bound::*;
pub use variable::*;

mod bound_value;
mod lit;
pub mod literals;
pub mod state;
mod var_bound;
mod variable;

/// Identifies an external writer to the model.
#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct WriterId(pub u8);

impl WriterId {
    pub fn new(num: impl Into<u8>) -> WriterId {
        WriterId(num.into())
    }

    pub fn cause(&self, cause: impl Into<u32>) -> Cause {
        Cause::inference(*self, cause)
    }
}
