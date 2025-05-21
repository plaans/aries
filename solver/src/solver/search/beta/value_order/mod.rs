mod dynamic;
mod lower_half;
mod max;
mod min;
mod upper_half;

use crate::core::Lit;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::Conflict;
use crate::solver::search::DecLvl;
use crate::solver::search::Explainer;

pub use crate::solver::search::beta::value_order::dynamic::Dynamic;
pub use crate::solver::search::beta::value_order::lower_half::LowerHalf;
pub use crate::solver::search::beta::value_order::max::Max;
pub use crate::solver::search::beta::value_order::min::Min;
pub use crate::solver::search::beta::value_order::upper_half::UpperHalf;

pub trait ValueOrder<Lbl: Label> {
    /// Return the literal to decide for the given variable.
    fn select(&mut self, var: VarRef, model: &Model<Lbl>) -> Lit;

    /// Function called each time a conflict occurs.
    fn conflict(
        &mut self,
        _clause: &Conflict,
        _model: &Model<Lbl>,
        _explainer: &mut dyn Explainer,
        _backtrack_level: DecLvl,
    ) {
    }
}

#[derive(Clone, Debug)]
pub enum ValueOrderKind {
    Min(Min),
    Max(Max),
    LowerHalf(LowerHalf),
    UpperHalf(UpperHalf),
    Dynamic(Dynamic),
}

impl<Lbl: Label> ValueOrder<Lbl> for ValueOrderKind {
    fn select(&mut self, var: VarRef, model: &Model<Lbl>) -> Lit {
        debug_assert!(!model.state.is_bound(var));
        match self {
            ValueOrderKind::Min(min) => min.select(var, model),
            ValueOrderKind::Max(max) => max.select(var, model),
            ValueOrderKind::LowerHalf(lower_half) => lower_half.select(var, model),
            ValueOrderKind::UpperHalf(upper_half) => upper_half.select(var, model),
            ValueOrderKind::Dynamic(dynamic) => dynamic.select(var, model),
        }
    }

    fn conflict(
        &mut self,
        clause: &Conflict,
        model: &Model<Lbl>,
        explainer: &mut dyn Explainer,
        backtrack_level: DecLvl,
    ) {
        match self {
            ValueOrderKind::Min(min) => min.conflict(clause, model, explainer, backtrack_level),
            ValueOrderKind::Max(max) => max.conflict(clause, model, explainer, backtrack_level),
            ValueOrderKind::LowerHalf(lower_half) => lower_half.conflict(clause, model, explainer, backtrack_level),
            ValueOrderKind::UpperHalf(upper_half) => upper_half.conflict(clause, model, explainer, backtrack_level),
            ValueOrderKind::Dynamic(dynamic) => dynamic.conflict(clause, model, explainer, backtrack_level),
        }
    }
}

impl Default for ValueOrderKind {
    fn default() -> Self {
        Self::Min(Min)
    }
}
