pub mod lifted_plan;
mod operation;

pub use operation::*;
use planx::RealValue;

pub struct Plan {
    operations: Vec<Operation<planx::Object>>,
    metric: Option<RealValue>,
}
