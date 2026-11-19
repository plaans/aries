//! General purpose analyses of a [`Model`](crate::Model), to support preprocessing transformations.

mod action_counts;
mod bounds;
mod domains;
mod dtg;
mod expressions;
mod guards;
mod numeric;
mod requirements;
mod timing;
mod usage;

pub use action_counts::*;
pub use bounds::*;
pub use domains::*;
pub use dtg::*;
pub use expressions::*;
pub use guards::*;
pub use numeric::*;
pub use requirements::*;
pub use timing::*;
pub use usage::*;
