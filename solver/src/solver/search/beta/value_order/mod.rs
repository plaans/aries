mod lower_half;
mod max;
mod min;
mod upper_half;

use crate::core::Lit;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;

pub use crate::solver::search::beta::value_order::lower_half::LowerHalf;
pub use crate::solver::search::beta::value_order::max::Max;
pub use crate::solver::search::beta::value_order::min::Min;
pub use crate::solver::search::beta::value_order::upper_half::UpperHalf;

pub trait ValueOrder {
    /// Return the literal to decide for the given variable.
    fn select<Lbl: Label>(&self, var: VarRef, model: &Model<Lbl>) -> Lit;
}

#[derive(Clone, Debug)]
pub enum ValueOrderKind {
    Min(Min),
    Max(Max),
    LowerHalf(LowerHalf),
    UpperHalf(UpperHalf),
}

impl ValueOrder for ValueOrderKind {
    fn select<Lbl: Label>(&self, var: VarRef, model: &Model<Lbl>) -> Lit {
        debug_assert!(!model.state.is_bound(var));
        match self {
            ValueOrderKind::Min(min) => min.select(var, model),
            ValueOrderKind::Max(max) => max.select(var, model),
            ValueOrderKind::LowerHalf(lower_half) => lower_half.select(var, model),
            ValueOrderKind::UpperHalf(upper_half) => upper_half.select(var, model),
        }
    }
}

impl Default for ValueOrderKind {
    fn default() -> Self {
        Self::Min(Min)
    }
}
