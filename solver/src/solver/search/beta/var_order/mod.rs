mod activity;
mod first_fail;
mod lexical;

use crate::backtrack::DecLvl;
use crate::core::state::Conflict;
use crate::core::state::Explainer;
use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
pub use crate::solver::search::beta::var_order::activity::Activity;
pub use crate::solver::search::beta::var_order::first_fail::FirstFail;
pub use crate::solver::search::beta::var_order::lexical::Lexical;

pub trait VarOrder<Lbl: Label> {
    /// Return the variable to branch on or None if no one is available.
    fn select(&self, model: &Model<Lbl>) -> Option<VarRef>;

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
pub enum VarOrderKind {
    Activity(Activity),
    Lexical(Lexical),
    FirstFail(FirstFail),
}

impl<Lbl: Label> VarOrder<Lbl> for VarOrderKind {
    fn select(&self, model: &Model<Lbl>) -> Option<VarRef> {
        match self {
            VarOrderKind::Activity(activity) => activity.select(model),
            VarOrderKind::Lexical(lexical) => lexical.select(model),
            VarOrderKind::FirstFail(first_fail) => first_fail.select(model),
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
            VarOrderKind::Activity(activity) => activity.conflict(clause, model, explainer, backtrack_level),
            VarOrderKind::Lexical(lexical) => lexical.conflict(clause, model, explainer, backtrack_level),
            VarOrderKind::FirstFail(first_fail) => first_fail.conflict(clause, model, explainer, backtrack_level),
        }
    }
}

impl Default for VarOrderKind {
    fn default() -> Self {
        Self::Lexical(Lexical)
    }
}

impl From<Activity> for VarOrderKind {
    fn from(value: Activity) -> Self {
        Self::Activity(value)
    }
}

impl From<Lexical> for VarOrderKind {
    fn from(value: Lexical) -> Self {
        Self::Lexical(value)
    }
}

impl From<FirstFail> for VarOrderKind {
    fn from(value: FirstFail) -> Self {
        Self::FirstFail(value)
    }
}
