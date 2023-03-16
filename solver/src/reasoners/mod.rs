use crate::backtrack::Backtrack;
use crate::core::state::{Cause, Explainer, InferenceCause};
use crate::core::state::{Domains, Explanation, InvalidUpdate};
use crate::core::Lit;
use crate::reasoners::cp::Cp;
use crate::reasoners::sat::SatSolver;
use crate::reasoners::stn::theory::StnTheory;
use std::fmt::{Display, Formatter};

pub mod cp;
pub mod sat;
pub mod stn;

/// Identifies an inference engine.
/// This ID is primarily used to identify the engine that caused each domain event.
#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub enum ReasonerId {
    Sat,
    Diff,
    Cp,
}

impl ReasonerId {
    pub fn cause(&self, cause: impl Into<u32>) -> Cause {
        Cause::inference(*self, cause)
    }
}

impl Display for ReasonerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use ReasonerId::*;
        write!(
            f,
            "{}",
            match self {
                Sat => "SAT",
                Diff => "DiffLog",
                Cp => "CP",
            }
        )
    }
}

pub trait Theory: Backtrack + Send + 'static {
    fn identity(&self) -> ReasonerId;

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction>;

    fn explain(&mut self, literal: Lit, context: u32, model: &Domains, out_explanation: &mut Explanation);

    fn print_stats(&self);

    fn clone_box(&self) -> Box<dyn Theory>;
}

#[derive(Debug)]
pub enum Contradiction {
    InvalidUpdate(InvalidUpdate),
    Explanation(Explanation),
}
impl From<InvalidUpdate> for Contradiction {
    fn from(empty: InvalidUpdate) -> Self {
        Contradiction::InvalidUpdate(empty)
    }
}
impl From<Explanation> for Contradiction {
    fn from(e: Explanation) -> Self {
        Contradiction::Explanation(e)
    }
}

pub(crate) const REASONERS: [ReasonerId; 3] = [ReasonerId::Sat, ReasonerId::Diff, ReasonerId::Cp];

/// A set of inference modules for constraint propagation.
#[derive(Clone)]
pub struct Reasoners {
    pub sat: SatSolver,
    pub diff: StnTheory,
    pub cp: Cp,
}
impl Reasoners {
    pub fn new() -> Self {
        Reasoners {
            sat: SatSolver::new(ReasonerId::Sat),
            diff: StnTheory::new(Default::default()),
            cp: Cp::new(ReasonerId::Cp),
        }
    }

    pub fn reasoner(&self, id: ReasonerId) -> &dyn Theory {
        match id {
            ReasonerId::Sat => &self.sat,
            ReasonerId::Diff => &self.diff,
            ReasonerId::Cp => &self.cp,
        }
    }

    pub fn reasoner_mut(&mut self, id: ReasonerId) -> &mut dyn Theory {
        match id {
            ReasonerId::Sat => &mut self.sat,
            ReasonerId::Diff => &mut self.diff,
            ReasonerId::Cp => &mut self.cp,
        }
    }

    pub fn writers(&self) -> &'static [ReasonerId] {
        &REASONERS
    }

    pub fn theories(&self) -> impl Iterator<Item = (ReasonerId, &dyn Theory)> + '_ {
        self.writers().iter().map(|w| (*w, self.reasoner(*w)))
    }
}

impl Default for Reasoners {
    fn default() -> Self {
        Self::new()
    }
}

impl Explainer for Reasoners {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &Domains, explanation: &mut Explanation) {
        self.reasoner_mut(cause.writer)
            .explain(literal, cause.payload, model, explanation)
    }
}
