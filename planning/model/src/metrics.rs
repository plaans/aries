use crate::ExprId;

/// Objective of the planning problem: minimize/maximize a given expression
#[derive(Clone, Copy, Debug)]
pub enum Metric {
    Minimize(ExprId),
    Maximize(ExprId),
}
