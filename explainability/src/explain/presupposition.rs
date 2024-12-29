use std::sync::Arc;

use aries::backtrack::{Backtrack, DecLvl};
use aries::model::{Label, Model};
use aries::reasoners::stn::theory::{StnConfig, TheoryPropagationLevel};
use aries::solver::Solver;
use itertools::Itertools;

use crate::explain::{Query, Situation};

pub struct Presupposition<Lbl: Label> {
    pub kind: PresuppositionKind,
    pub model: Arc<Model<Lbl>>,
    pub situ: Situation,
    pub query: Query,
}

pub struct UnmetPresupposition<Lbl: Label> {
    presupposition: Presupposition<Lbl>,
    cause: UnmetPresuppositionCause,
}

pub enum PresuppositionKind {
    ModelSituUnsatWithQuery,
    ModelSituSatWithQuery,
    ModelSituNotEntailQuery,
    ModelSituEntailQuery,
}

pub enum UnmetPresuppositionCause {
    ModelSituUnsat,
    ModelSituQueryUnsat,
    ModelSituQuerySat,
    ModelSituNegQueryUnsat,
    ModelSituNegQuerySat,
}

