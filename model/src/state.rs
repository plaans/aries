mod cause;
mod domain;
mod domains;
mod event;
mod explanation;
mod int_domains;
mod presence_graph;

pub use cause::*;
pub use domain::*;
pub use domains::*;
pub use event::*;
pub use explanation::*;
pub use int_domains::*;

use crate::bounds::Lit;

/// Represents a triggered event of setting a conflicting literal.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct InvalidUpdate(pub Lit, pub Origin);
