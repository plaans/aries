use crate::collection::index_map::{IndexMap, ToIndex};
use crate::collection::Range;
use crate::core::all::{BVar, Lit};
use std::fmt::{Display, Error, Formatter};
use std::num::NonZeroU32;
use std::ops::{Index, IndexMut};

pub struct ClausesParams {
    cla_inc: f64,
    cla_decay: f64,
}
impl Default for ClausesParams {
    fn default() -> Self {
        ClausesParams {
            cla_inc: 1_f64,
            cla_decay: 0.999_f64,
        }
    }
}

pub struct Clause {
    activity: f64,
    learnt: bool,
    pub disjuncts: Vec<Lit>,
}
impl Clause {
    pub fn new(lits: &[Lit], learnt: bool) -> Self {
        assert!(lits.len() >= 2);
        Clause {
            activity: 0_f64,
            learnt,
            disjuncts: Vec::from(lits),
        }
    }

    // TODO: remove usage, in general a clause should be just [Lit]
    pub fn from(lits: &[i32]) -> Self {
        let mut x = Vec::with_capacity(lits.len());
        for &l in lits {
            let lit = if l > 0 {
                BVar::from_bits(l as u32).true_lit()
            } else if l < 0 {
                BVar::from_bits((-l) as u32).false_lit()
            } else {
                panic!()
            };
            x.push(lit);
        }
        Clause {
            activity: 0_f64,
            learnt: false,
            disjuncts: x,
        }
    }
}
impl Display for Clause {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "[")?;
        for i in 0..self.disjuncts.len() {
            if i != 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", self.disjuncts[i])?;
        }
        write!(f, "]")
    }
}

#[derive(PartialOrd, PartialEq, Debug, Clone, Copy)]
pub struct ClauseId {
    id: u32,
    /// Marker set by the clause DB to track the version of the database.
    /// This is for debugging purposes only to make sure we can detect outdated pointers.
    version: std::num::NonZeroU32,
}

impl ClauseId {
    pub fn new(id: u32, version: NonZeroU32) -> Self {
        ClauseId { id, version }
    }
}

impl crate::collection::Next for ClauseId {
    fn next_n(self, n: usize) -> Self {
        ClauseId::new(self.id + n as u32, self.version)
    }
}

impl Display for ClauseId {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.id)
    }
}

impl ToIndex for ClauseId {
    fn to_index(&self) -> usize {
        self.id as usize
    }
    fn first_index() -> usize {
        0
    }
}

pub struct ClauseDB {
    params: ClausesParams,
    clauses: IndexMap<ClauseId, Clause>,
    version: std::num::NonZeroU32,
}

impl ClauseDB {
    pub fn new(params: ClausesParams) -> ClauseDB {
        ClauseDB {
            params,
            clauses: IndexMap::empty(),
            version: NonZeroU32::new(1).unwrap(),
        }
    }

    pub fn add_clause(&mut self, cl: Clause) -> ClauseId {
        let id = self.clauses.push(cl);
        ClauseId::new(id as u32, self.version)
    }

    pub fn num_clauses(&self) -> usize {
        self.clauses.len()
    }

    pub fn all_clauses(&self) -> Range<ClauseId> {
        Range::new(
            ClauseId::new(0, self.version),
            ClauseId::new((self.num_clauses() - 1) as u32, self.version),
        )
    }

    pub fn bump_activity(&mut self, cl: ClauseId) {
        self[cl].activity += self.params.cla_inc;
        if self[cl].activity > 1e100_f64 {
            self.rescale_activities()
        }
    }

    pub fn decay_activities(&mut self) {
        self.params.cla_inc /= self.params.cla_decay;
    }

    fn rescale_activities(&mut self) {
        self.clauses
            .values_mut()
            .for_each(|v| v.activity *= 1e-100_f64);
        self.params.cla_inc *= 1e-100_f64;
    }
}

impl Index<ClauseId> for ClauseDB {
    type Output = Clause;
    fn index(&self, k: ClauseId) -> &Self::Output {
        debug_assert!(k.version == self.version, "Using outdated clause ID");
        &self.clauses[k]
    }
}
impl IndexMut<ClauseId> for ClauseDB {
    fn index_mut(&mut self, k: ClauseId) -> &mut Self::Output {
        debug_assert!(k.version == self.version, "Using outdated clause ID");
        &mut self.clauses[k]
    }
}
