use crate::backtrack::Backtrack;
use crate::core::Lit;
use crate::core::state::{Cause, DomainsSnapshot, Explainer, InferenceCause};
use crate::core::state::{Domains, Explanation, InvalidUpdate};
use crate::reasoners::cp::Cp;
use crate::reasoners::eq::SplitEqTheory;
use crate::reasoners::sat::SatSolver;
use crate::reasoners::stn::theory::StnTheory;
use crate::reasoners::tautologies::Tautologies;
use std::fmt::{Display, Formatter};

pub mod cp;
pub mod eq;
pub mod sat;
pub mod stn;
pub mod tautologies;

/// Identifies an inference engine.
/// This ID is primarily used to identify the engine that caused each domain event.
#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub enum ReasonerId {
    Sat,
    Diff,
    Cp,
    Eq(u16),
    Tautologies,
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
                Eq(_) => "Equality",
                Cp => "CP",
                Tautologies => "Optim",
            }
        )
    }
}

pub trait Theory: Backtrack + Send + 'static {
    fn identity(&self) -> ReasonerId;

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction>;

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    );

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

/// List of reasoners used notably for propagation.
///
/// SAT should always be first because we should not allow anything to happen between
/// the moment a clause is learned and the moment it is is propagated.
pub(crate) const REASONERS: [ReasonerId; 5] = [
    ReasonerId::Sat,
    ReasonerId::Tautologies,
    ReasonerId::Diff,
    ReasonerId::Eq(0),
    ReasonerId::Cp,
];

/// A set of inference modules for constraint propagation.
#[derive(Clone)]
pub struct Reasoners {
    pub sat: SatSolver,
    pub diff: StnTheory,
    pub eq: SplitEqTheory,
    pub cp: Cp,
    pub tautologies: Tautologies,
}
impl Reasoners {
    pub fn new() -> Self {
        Reasoners {
            sat: SatSolver::new(ReasonerId::Sat),
            diff: StnTheory::new(Default::default()),
            eq: Default::default(),
            cp: Cp::new(ReasonerId::Cp),
            tautologies: Tautologies::default(),
        }
    }

    pub fn reasoner(&self, id: ReasonerId) -> &dyn Theory {
        match id {
            ReasonerId::Sat => &self.sat,
            ReasonerId::Diff => &self.diff,
            ReasonerId::Eq(_) => &self.eq,
            ReasonerId::Cp => &self.cp,
            ReasonerId::Tautologies => &self.tautologies,
        }
    }

    pub fn reasoner_mut(&mut self, id: ReasonerId) -> &mut dyn Theory {
        match id {
            ReasonerId::Sat => &mut self.sat,
            ReasonerId::Diff => &mut self.diff,
            ReasonerId::Eq(_) => &mut self.eq,
            ReasonerId::Cp => &mut self.cp,
            ReasonerId::Tautologies => &mut self.tautologies,
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
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &DomainsSnapshot, explanation: &mut Explanation) {
        self.reasoner_mut(cause.writer)
            .explain(literal, cause, model, explanation)
    }
}
