use crate::core::state::Cause;
pub use bound_value::*;
pub use lit::*;
use std::fmt::{Display, Formatter};
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
pub enum WriterId {
    Sat,
    Diff,
    Cp,
}

impl WriterId {
    pub fn cause(&self, cause: impl Into<u32>) -> Cause {
        Cause::inference(*self, cause)
    }
}

impl Display for WriterId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use WriterId::*;
        write!(
            f,
            "{}",
            match self {
                Sat => "SAT",
                Diff => "DiffLog",
                Cp => "CP",
            }
        )
    }
}
