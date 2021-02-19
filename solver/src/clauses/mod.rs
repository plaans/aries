use aries_backtrack::EventIndex;
use aries_collections::ref_store::RefVec;
use aries_collections::*;
use aries_model::bounds::{Bound, Disjunction};
use itertools::Itertools;
use std::cmp::Ordering::Equal;
use std::fmt::{Debug, Display, Error, Formatter};
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

/// A clause represents a disjunction of literals together with some metadata needed to decide whether
/// it can be removed from a clause database.
///
/// The layout is optimized for the workflow of a sat propagation. In a typical workflow, the unwatched
/// literals are only accessed a fraction of time the watches are accessed (between 10% and 40% on most
/// benchmarks I have test). Even when accessed, the watches will be accessed first, and only under some condition
/// that depend on the watches will the unwatched literals be accessed.
///
/// The memory layout tries to optimize for this pattern by having a fixed size struct holding the watches.
/// This allows to have all watches in a single array that should mostly fit in the L1 cache.
/// Access to the unwatched literals occurring later in the propagation it should provide some time for
/// the CPU to retrieve the unwatched literals from higher layers in the caches.
pub struct Clause {
    pub activity: f64,
    pub learnt: bool,
    pub watch1: Bound,
    pub watch2: Bound,
    pub unwatched: Box<[Bound]>,
}
impl Clause {
    /// Creates a new clause from the disjunctive set of literals.
    /// It is assumed that the set of literals is non empty.
    /// No clean up of the clause will be made to remove redundant literals.
    pub fn new(lits: Disjunction, learnt: bool) -> Self {
        let lits = Vec::from(lits);
        match lits.len() {
            0 => panic!(),
            1 => Clause {
                activity: 0_f64,
                learnt,
                watch1: lits[0],
                watch2: lits[0],
                unwatched: [].into(),
            },
            _ => Clause {
                activity: 0f64,
                learnt,
                watch1: lits[0],
                watch2: lits[1],
                unwatched: lits[2..].into(),
            },
        }
    }

    /// True if the clause has no literals
    pub fn is_empty(&self) -> bool {
        // Always false, would panic in constructor otherwise
        false
    }

    /// Number of literals in the clause.
    pub fn len(&self) -> usize {
        if self.watch1 == self.watch2 {
            1
        } else {
            self.unwatched.len() + 2
        }
    }

    /// Exchange the two watches.
    pub fn swap_watches(&mut self) {
        std::mem::swap(&mut self.watch1, &mut self.watch2);
    }

    /// Sets the first watch to the given unwatched literal. The previously
    /// watched literal will be put in the unwatched list.
    pub fn set_watch1(&mut self, unwatched_index: usize) {
        std::mem::swap(&mut self.watch1, &mut self.unwatched[unwatched_index]);
    }

    /// Sets the second watch to the given unwatched literal. The previously
    /// watched literal will be put in the unwatched list.
    pub fn set_watch2(&mut self, unwatched_index: usize) {
        std::mem::swap(&mut self.watch2, &mut self.unwatched[unwatched_index]);
    }

    /// Returns an iterator over the literals of the clause.
    /// The two watches will come first.
    pub fn literals(&self) -> impl Iterator<Item = Bound> + '_ {
        Literals {
            next: 0,
            len: self.len(),
            cl: self,
        }
    }

    /// Select the two literals to watch and move them to the first 2 literals of the clause.
    ///
    /// After the method completion `watch1` will be the element with the highest priority and `watch2` the one with
    /// the second highest priority. Order of other elements is undefined.
    ///
    /// Priority is defined as follows:
    ///   - TRUE literals
    ///   - UNDEF literals
    ///   - FALSE Literal, prioritizing those with the highest decision level
    ///   - left most literal in the original clause (to avoid swapping two literals with the same priority)
    pub fn move_watches_front(
        &mut self,
        value_of: impl Fn(Bound) -> Option<bool>,
        implying_event: impl Fn(Bound) -> Option<EventIndex>,
    ) {
        let priority = |lit: Bound| match value_of(lit) {
            Some(true) => usize::MAX,
            None => usize::MAX - 1,
            Some(false) => implying_event(!lit).map(|id| usize::from(id) + 1).unwrap_or(0),
        };
        debug_assert!(self.len() >= 2);
        let mut lvl0 = priority(self.watch1);
        let mut lvl1 = priority(self.watch2);
        if lvl1 > lvl0 {
            std::mem::swap(&mut lvl0, &mut lvl1);
            self.swap_watches();
        }
        for i in 0..self.unwatched.len() {
            let lvl = priority(self.unwatched[i]);
            if lvl > lvl1 {
                lvl1 = lvl;
                self.set_watch2(i);
                if lvl > lvl0 {
                    lvl1 = lvl0;
                    lvl0 = lvl;
                    self.swap_watches();
                }
            }
        }
        debug_assert_eq!(lvl0, priority(self.watch1));
        debug_assert_eq!(lvl1, priority(self.watch2));
        debug_assert!(lvl0 >= lvl1);
        debug_assert!(self.unwatched.iter().all(|l| lvl1 >= priority(*l)));
    }
}
impl Display for Clause {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "[")?;
        for (i, lit) in self.literals().enumerate() {
            if i != 0 {
                write!(f, " ")?;
            }
            write!(f, "{:?}", lit)?;
        }
        write!(f, "]")
    }
}

/// An iterator over the literals in the clause. Watches come first and the other literals
/// are in an arbitrary order.
pub struct Literals<'a> {
    next: usize,
    len: usize,
    cl: &'a Clause,
}
impl<'a> Iterator for Literals<'a> {
    type Item = Bound;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.len {
            self.next += 1;
            match self.next {
                1 => Some(self.cl.watch1),
                2 => Some(self.cl.watch2),
                i => Some(self.cl.unwatched[i - 3]),
            }
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a Clause {
    type Item = Bound;
    type IntoIter = Literals<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Literals {
            next: 0,
            len: self.len(),
            cl: self,
        }
    }
}

create_ref_type!(ClauseId);

impl Display for ClauseId {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", usize::from(*self))
    }
}

pub struct ClauseDB {
    params: ClausesParams,
    num_fixed: usize,
    num_clauses: usize, // number of clause that are not learnt
    first_possibly_free: usize,
    clauses: RefVec<ClauseId, Option<Clause>>,
}

impl ClauseDB {
    pub fn new(params: ClausesParams) -> ClauseDB {
        ClauseDB {
            params,
            num_fixed: 0,
            num_clauses: 0,
            first_possibly_free: 0,
            clauses: RefVec::new(),
        }
    }

    pub fn add_clause(&mut self, cl: Clause) -> ClauseId {
        self.num_clauses += 1;
        if !cl.learnt {
            self.num_fixed += 1;
        }

        debug_assert!((0..self.first_possibly_free).all(|i| self.clauses[ClauseId::from(i)].is_some()));

        let first_free_spot = self
            .clauses
            .keys()
            .dropping(self.first_possibly_free.saturating_sub(1))
            .find(|&k| self.clauses[k].is_none());

        // insert in first free spot
        let id = match first_free_spot {
            Some(id) => {
                debug_assert!(self.clauses[id].is_none());
                self.clauses[id] = Some(cl);
                id
            }
            None => {
                debug_assert_eq!(self.num_clauses - 1, self.clauses.len()); // note: we have already incremented the clause counts
                                                                            // no free spaces push at the end
                self.clauses.push(Some(cl))
            }
        };
        self.first_possibly_free = usize::from(id) + 1;

        id
    }

    pub fn num_clauses(&self) -> usize {
        self.num_clauses
    }
    pub fn num_learnt(&self) -> usize {
        self.num_clauses - self.num_fixed
    }

    pub fn all_clauses(&self) -> impl Iterator<Item = ClauseId> + '_ {
        ClauseId::first(self.clauses.len()).filter(move |&cl_id| self.clauses[cl_id].is_some())
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
        self.clauses.keys().for_each(|k| match &mut self.clauses[k] {
            Some(clause) => clause.activity *= 1e-100_f64,
            None => (),
        });
        self.params.cla_inc *= 1e-100_f64;
    }

    pub fn reduce_db<F: Fn(ClauseId) -> bool>(&mut self, locked: F, remove_watch: &mut impl FnMut(ClauseId, Bound)) {
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
            let cl = self.clauses[id].as_ref().unwrap();
            if !cl.is_empty() {
                remove_watch(id, !cl.watch1);
            }
            if cl.len() >= 2 {
                remove_watch(id, !cl.watch2);
            }
            self.clauses[id] = None;
            self.num_clauses -= 1;
        });

        // make sure we search for free spots from the beginning
        self.first_possibly_free = 0;
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
