mod geometric;
mod never;

use crate::backtrack::DecLvl;
use crate::core::state::Conflict;
use crate::core::state::Explainer;
use crate::model::Label;
use crate::model::Model;
pub use crate::solver::search::beta::restart::geometric::Geometric;
pub use crate::solver::search::beta::restart::never::Never;

pub trait Restart<Lbl: Label> {
    /// Return true if the solver should restart.
    fn restart(&mut self) -> bool;

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
pub enum RestartKind {
    Geometric(Geometric),
    Never(Never),
}

impl<Lbl: Label> Restart<Lbl> for RestartKind {
    fn restart(&mut self) -> bool {
        match self {
            RestartKind::Geometric(geometric) => <Geometric as Restart<Lbl>>::restart(geometric),
            RestartKind::Never(never) => <Never as Restart<Lbl>>::restart(never),
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
            RestartKind::Geometric(geometric) => geometric.conflict(clause, model, explainer, backtrack_level),
            RestartKind::Never(never) => never.conflict(clause, model, explainer, backtrack_level),
        }
    }
}

impl Default for RestartKind {
    fn default() -> Self {
        Self::Geometric(Default::default())
    }
}
