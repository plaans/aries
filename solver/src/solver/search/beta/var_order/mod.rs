mod first_fail;
mod lexical;

use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
pub use crate::solver::search::beta::var_order::first_fail::FirstFail;
pub use crate::solver::search::beta::var_order::lexical::Lexical;

pub trait VarOrder {
    /// Return the variable to branch on or None if no one is available.
    fn select<Lbl: Label>(&self, model: &Model<Lbl>) -> Option<VarRef>;
}

#[derive(Clone, Debug)]
pub enum VarOrderKind {
    Lexical(Lexical),
    FirstFail(FirstFail),
}

impl VarOrder for VarOrderKind {
    fn select<Lbl: Label>(&self, model: &Model<Lbl>) -> Option<VarRef> {
        match self {
            VarOrderKind::Lexical(lexical) => lexical.select(model),
            VarOrderKind::FirstFail(first_fail) => first_fail.select(model),
        }
    }
}

impl Default for VarOrderKind {
    fn default() -> Self {
        Self::Lexical(Lexical)
    }
}
