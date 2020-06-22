use aries_collections::index_map::{IndexMap, ToIndex};
use aries_collections::*;
use crate::sat::all::Lit;
use std::cmp::Ordering::Equal;
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
    pub activity: f64,
    pub learnt: bool,
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

#[derive(Eq, Hash, PartialOrd, PartialEq, Debug, Clone, Copy)]
pub struct ClauseId {
    id: NonZeroU32,
}

impl ClauseId {
    pub fn new(id: u32) -> Self {
        ClauseId {
            id: NonZeroU32::new(id).unwrap(),
        }
    }
}

impl aries_collections::Next for ClauseId {
    fn next_n(self, n: usize) -> Self {
        ClauseId::new(self.id.get() + n as u32)
    }
}
impl aries_collections::MinVal for ClauseId {
    fn min_value() -> Self {
        ClauseId::new(ClauseId::first_index() as u32)
    }
}

impl Display for ClauseId {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.id.get())
    }
}

impl ToIndex for ClauseId {
    fn to_index(&self) -> usize {
        self.id.get() as usize
    }
    fn first_index() -> usize {
        1
    }
}

pub struct ClauseDB {
    params: ClausesParams,
    num_fixed: usize,
    num_clauses: usize, // number of clause that are not learnt
    first_possibly_free: usize,
    clauses: IndexMap<ClauseId, Option<Clause>>,
}

impl ClauseDB {
    pub fn new(params: ClausesParams) -> ClauseDB {
        ClauseDB {
            params,
            num_fixed: 0,
            num_clauses: 0,
            first_possibly_free: ClauseId::first_index(),
            clauses: IndexMap::new_with(0, || None),
        }
    }

    pub fn add_clause(&mut self, cl: Clause) -> ClauseId {
        self.num_clauses += 1;
        if !cl.learnt {
            self.num_fixed += 1;
        }

        debug_assert!((ClauseId::first_index()..self.first_possibly_free).all(|i| self.clauses.values[i].is_some()));

        // insert in first free spot
        let id = match self.clauses.scan(self.first_possibly_free, |v| v.is_none()) {
            Some(id) => {
                debug_assert!(id > 0);
                debug_assert!(self.clauses.values[id].is_none());
                self.clauses.overwrite(id, Some(cl));
                id
            }
            None => {
                debug_assert_eq!(self.num_clauses - 1, self.clauses.num_elems()); // note: we have already incremented the clause counts
                                                                                  // no free spaces push at the end
                self.clauses.push(Some(cl))
            }
        };
        self.first_possibly_free = id + 1;

        ClauseId::new(id as u32)
    }

    pub fn num_clauses(&self) -> usize {
        self.num_clauses
    }
    pub fn num_learnt(&self) -> usize {
        self.num_clauses - self.num_fixed
    }

    pub fn all_clauses<'a>(&'a self) -> impl Iterator<Item = ClauseId> + 'a {
        ClauseId::first(self.clauses.num_elems()).filter(move |&cl_id| self.clauses[cl_id].is_some())
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
        self.clauses.values_mut().for_each(|v| match v {
            Some(clause) => clause.activity *= 1e-100_f64,
            None => (),
        });
        self.params.cla_inc *= 1e-100_f64;
    }

    pub fn reduce_db<F: Fn(ClauseId) -> bool>(&mut self, locked: F, watches: &mut IndexMap<Lit, Vec<ClauseId>>) {
        let mut clauses = self
            .all_clauses()
            .filter_map(|cl_id| match &self.clauses[cl_id] {
                Some(clause) if clause.learnt && !locked(cl_id) => Some((cl_id, clause.activity)),
                _ => None,
            })
            .collect::<Vec<_>>();
        clauses.sort_by(|&a, &b| a.1.partial_cmp(&b.1).unwrap_or(Equal));
        // remove half removable
        clauses.iter().take(clauses.len() / 2).for_each(|&(id, _)| {
            let watched = &self[id].disjuncts[0..=1];
            for l in watched {
                debug_assert!(watches[!*l].contains(&id), "");
                watches[!*l].retain(|&clause| clause != id);
            }

            self.clauses.overwrite(id.to_index(), None);
            self.num_clauses -= 1;
        });

        // make sure we search for free spots from the beginning
        self.first_possibly_free = ClauseId::first_index();
    }
}

impl Index<ClauseId> for ClauseDB {
    type Output = Clause;
    fn index(&self, k: ClauseId) -> &Self::Output {
        self.clauses[k].as_ref().unwrap()
    }
}
impl IndexMut<ClauseId> for ClauseDB {
    fn index_mut(&mut self, k: ClauseId) -> &mut Self::Output {
        self.clauses[k].as_mut().unwrap()
    }
}
