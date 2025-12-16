use std::fmt::Display;

use crate::{Env, ExprId};

/// Objective of the planning problem: minimize/maximize a given expression
#[derive(Clone, Copy, Debug)]
pub enum Metric {
    Minimize(ExprId),
    Maximize(ExprId),
}

impl Display for Env<'_, &Metric> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.elem {
            Metric::Minimize(expr_id) => write!(f, "minimize: {}", self.env / *expr_id),
            Metric::Maximize(expr_id) => write!(f, "maximize: {}", self.env / *expr_id),
        }
    }
}
