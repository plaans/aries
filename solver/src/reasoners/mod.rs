use itertools::Itertools;

use crate::backtrack::Backtrack;
use crate::core::Lit;
use crate::core::state::{Cause, DomainsSnapshot, Explainer, InferenceCause};
use crate::core::state::{Domains, Explanation, InvalidUpdate};
use crate::reasoners::cp::Cp;
use crate::reasoners::sat::SatSolver;
use crate::reasoners::stn::StnTheory;
use crate::reasoners::tautologies::Tautologies;
use std::fmt::{Display, Formatter};

pub mod cp;
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
    Tautologies,
    Extra(u8),
}

impl ReasonerId {
    pub fn cause(&self, cause: impl Into<u32>) -> Cause {
        Cause::inference(*self, cause)
    }
}

impl Display for ReasonerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use ReasonerId::*;
        let mut _extra_str = String::new();
        write!(
            f,
            "{}",
            match self {
                Sat => "SAT",
                Diff => "DiffLog",
                Cp => "CP",
                Tautologies => "Optim",
                Extra(i) => {
                    _extra_str = format!("Extra({i})");
                    &_extra_str
                }
            }
        )
    }
}

pub trait Theory: Backtrack + Send {
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
pub(crate) const REASONERS: [ReasonerId; 4] = [
    ReasonerId::Sat,
    ReasonerId::Tautologies,
    ReasonerId::Diff,
    ReasonerId::Cp,
];

pub(crate) struct ReasonersTheories {
    pub sat: SatSolver,
    pub diff: StnTheory,
    pub cp: Cp,
    pub tautologies: Tautologies,
    pub extra: Vec<Box<dyn Theory>>,
}
impl Clone for ReasonersTheories {
    fn clone(&self) -> Self {
        Self {
            sat: self.sat.clone(),
            diff: self.diff.clone(),
            cp: self.cp.clone(),
            tautologies: self.tautologies.clone(),
            extra: self.extra.iter().map(|th| th.clone_box()).collect(),
        }
    }
}
impl ReasonersTheories {
    pub fn new() -> Self {
        ReasonersTheories {
            sat: SatSolver::new(ReasonerId::Sat),
            diff: StnTheory::new(Default::default()),
            cp: Cp::new(ReasonerId::Cp),
            tautologies: Tautologies::default(),
            extra: vec![],
        }
    }
    pub fn with_extra(extra: Vec<Box<dyn Theory>>) -> Self {
        assert!(
            extra
                .iter()
                .map(|r| r.identity())
                .all(|rid| matches!(rid, ReasonerId::Extra(_)))
        );
        assert!(extra.iter().map(|r| r.identity()).all_unique());

        ReasonersTheories {
            sat: SatSolver::new(ReasonerId::Sat),
            diff: StnTheory::new(Default::default()),
            cp: Cp::new(ReasonerId::Cp),
            tautologies: Tautologies::default(),
            extra,
        }
    }
    pub fn get(&self, id: ReasonerId) -> &dyn Theory {
        match id {
            ReasonerId::Sat => &self.sat,
            ReasonerId::Diff => &self.diff,
            ReasonerId::Cp => &self.cp,
            ReasonerId::Tautologies => &self.tautologies,
            ReasonerId::Extra(id) => self.extra.get(id as usize).unwrap().as_ref(),
        }
    }
    pub fn get_mut(&mut self, id: ReasonerId) -> &mut dyn Theory {
        match id {
            ReasonerId::Sat => &mut self.sat,
            ReasonerId::Diff => &mut self.diff,
            ReasonerId::Cp => &mut self.cp,
            ReasonerId::Tautologies => &mut self.tautologies,
            ReasonerId::Extra(id) => self.extra.get_mut(id as usize).unwrap().as_mut(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ReasonersWriters {
    writers: Vec<ReasonerId>,
}
impl ReasonersWriters {
    pub fn new() -> Self {
        Self {
            writers: REASONERS.to_vec(),
        }
    }
    pub fn with_extra(extra: &Vec<Box<dyn Theory>>) -> Self {
        assert!(
            extra
                .iter()
                .map(|r| r.identity())
                .all(|rid| matches!(rid, ReasonerId::Extra(_)))
        );
        assert!(extra.iter().map(|r| r.identity()).all_unique());

        Self {
            writers: REASONERS
                .into_iter()
                .chain(extra.iter().map(|r| r.identity()))
                .collect(),
        }
    }
    pub fn get(&self) -> &[ReasonerId] {
        &self.writers
    }
}

/// A set of inference modules for constraint propagation.
#[derive(Clone)]
pub struct Reasoners {
    pub(crate) writers: ReasonersWriters,
    pub(crate) theories: ReasonersTheories,
}
impl Reasoners {
    pub fn new() -> Self {
        Self {
            writers: ReasonersWriters::new(),
            theories: ReasonersTheories::new(),
        }
    }
    pub fn with_extra(extra: Vec<Box<dyn Theory>>) -> Self {
        Self {
            writers: ReasonersWriters::with_extra(&extra),
            theories: ReasonersTheories::with_extra(extra),
        }
    }

    pub fn sat(&mut self) -> &mut SatSolver {
        &mut self.theories.sat
    }
    pub fn diff(&mut self) -> &mut StnTheory {
        &mut self.theories.diff
    }
    pub fn cp(&mut self) -> &mut Cp {
        &mut self.theories.cp
    }
    pub fn tautologies(&mut self) -> &mut Tautologies {
        &mut self.theories.tautologies
    }
    pub fn extra(&mut self) -> &mut [Box<dyn Theory>] {
        &mut self.theories.extra
    }

    pub fn iter(&self) -> impl Iterator<Item = (ReasonerId, &dyn Theory)> + '_ {
        self.writers.get().iter().map(|w| (*w, self.theories.get(*w)))
    }
}

impl Default for Reasoners {
    fn default() -> Self {
        Self::new()
    }
}

impl Explainer for Reasoners {
    fn explain(&mut self, cause: InferenceCause, literal: Lit, model: &DomainsSnapshot, explanation: &mut Explanation) {
        self.theories
            .get_mut(cause.writer)
            .explain(literal, cause, model, explanation)
    }
}
