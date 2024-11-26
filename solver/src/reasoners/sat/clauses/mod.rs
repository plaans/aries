use crate::backtrack::EventIndex;
use crate::collections::ref_store::{RefMap, RefVec};
use crate::collections::seq::Seq;
use crate::core::literals::Disjunction;
use crate::core::Lit;
use crate::create_ref_type;
use env_param::EnvParam;
use itertools::Itertools;
use std::cmp::Ordering::Equal;
use std::fmt::{Debug, Display, Error, Formatter};
use std::ops::{Index, IndexMut};

pub static DEFAULT_LOCKED_LBD_LEVEL: EnvParam<u32> = EnvParam::new("ARIES_SAT_LBD_LOCK_LEVEL", "4");

#[derive(Clone)]
pub struct ClausesParams {
    pub cla_inc: f64,
    pub cla_decay: f64,
    /// All clauses whose Literal Block Distance (LBD) is LEQ than this one will not be removed
    /// when reducing the DB. Note that the LBD may evolve overtime and is typically reevaluate on unit propagation
    pub locked_lbd_level: u32,
}
impl Default for ClausesParams {
    fn default() -> Self {
        ClausesParams {
            cla_inc: 1_f64,
            cla_decay: 0.999_f64,
            locked_lbd_level: DEFAULT_LOCKED_LBD_LEVEL.get(),
        }
    }
}

#[derive(Copy, Clone)]
struct ClauseMetadata {
    pub activity: f64,
    pub lbd: u32,
    pub learnt: bool,
}

/// A clause represents a disjunction of literals `a || b || c`. It may also contain a `scope` literal defining
/// when the clause needs to hold. A scoped clause may be interpreted as `scope => (a || b || c)`,
/// or equivalently `!scope || a || b || c`
///
/// The layout is optimized for the workflow of sat propagation. In a typical workflow, the unwatched
/// literals are only accessed a fraction of time the watches are accessed (between 10% and 40% on most
/// benchmarks I have test). Even when accessed, the watches will be accessed first, and only under some condition
/// that depend on the watches will the unwatched literals be accessed.
///
/// The memory layout tries to optimize for this pattern by having a fixed size struct holding the watches.
/// This allows to have all watches in a single array that should mostly fit in the L1 cache.
/// Access to the unwatched literals occurring later in the propagation it should provide some time for
/// the CPU to retrieve the unwatched literals from higher layers in the caches.
#[derive(Clone)]
pub struct Clause {
    pub watch1: Lit,
    pub watch2: Lit,
    pub unwatched: Box<[Lit]>,
}
impl Clause {
    /// Creates a new clause from the disjunctive set of literals.
    /// It is assumed that the set of literals is non empty.
    /// No clean up of the clause will be made to remove redundant literals.
    pub fn new(lits: Disjunction) -> Self {
        Self::new_scoped(lits, Lit::TRUE)
    }

    /// Creates a new scoped clause, that can be eagerly propagated.
    /// The caller MUST ensure that all literals are optional and only present when `scope` is true.
    pub fn new_scoped(lits: Disjunction, scope: Lit) -> Self {
        // redefine the lits and scope by moving the scope into the clause

        // add !scope to clause
        let mut lits = lits.to_vec();
        if scope != Lit::TRUE {
            lits.push(!scope);
        }
        let lits = Disjunction::new(lits);

        let lits = Vec::from(lits);

        match lits.len() {
            0 => panic!(),
            1 => Clause {
                watch1: lits[0],
                watch2: lits[0],
                unwatched: [].into(),
            },
            _ => {
                debug_assert_ne!(lits[0], lits[1]);
                Clause {
                    watch1: lits[0],
                    watch2: lits[1],
                    unwatched: lits[2..].into(),
                }
            }
        }
    }

    /// True if the clause has no literals
    pub fn is_empty(&self) -> bool {
        // Always false, would panic in constructor otherwise
        false
    }

    /// Return true if the clause has a single literal
    pub fn has_single_literal(&self) -> bool {
        // We have the invariant that the clause is not empty.
        // Our encoding of a unit clause is to have the two watches on hte same literal.
        self.watch1 == self.watch2
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
    pub fn literals(&self) -> impl Iterator<Item = Lit> + '_ {
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
        value_of: impl Fn(Lit) -> Option<bool>,
        implying_event: impl Fn(Lit) -> Option<EventIndex>,
        presence: impl Fn(Lit) -> Lit,
    ) {
        let priority = |lit: Lit| match value_of(lit) {
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

        if value_of(self.watch1) == Some(true) {
            // clause is satisfied, leave the watches untouched (true literals would be the watches)
            return;
        }
        debug_assert_ne!(value_of(self.watch1), Some(true));
        debug_assert_ne!(value_of(self.watch2), Some(true));

        if self.watch1 == !presence(self.watch2) {
            self.swap_watches()
        } else if self.watch2 == !presence(self.watch1) {
        } else {
            // the two watches are not fusable, we are done
            return;
        }
        debug_assert_eq!(self.watch2, !presence(self.watch1));
        // the two watches are fusable: `watch2` represents the absence of `watch1`

        // take the highest priority literal
        let replacement = self
            .unwatched
            .iter()
            .copied()
            .enumerate()
            .sorted_by_key(|(_i, l)| priority(*l))
            .find(|(_i, l)| value_of(*l) != Some(false));
        if let Some((i, _lit)) = replacement {
            // we have a literal (distinct from the presence of the first watch) that is unset, place it as the second watch
            self.set_watch2(i);
        } else {
            // no replacement, which means the clause is unit. Leave things as they are.
        }
        // maintain the invariant that the first watch has higher priority (used to determine the status of a clause))
        if priority(self.watch1) < priority(self.watch2) {
            self.swap_watches()
        }
        debug_assert!(priority(self.watch1) >= priority(self.watch2));
    }
}
impl Display for Clause {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "[")?;
        for (i, lit) in self.literals().enumerate() {
            if i != 0 {
                write!(f, " ")?;
            }
            write!(f, "{lit:?}")?;
        }
        write!(f, "]")
    }
}
impl Debug for Clause {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
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
    type Item = Lit;

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
    type Item = Lit;
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

#[derive(Clone)]
pub struct ClauseDb {
    pub params: ClausesParams,
    /// Number of clauses that are not learnt and cannot be removed from the database.
    num_fixed: usize,
    /// Total number of clauses.
    num_clauses: usize,
    // Number of learnt clauses that are locked (should not be removed) because of a low LBD indice.
    num_learnt_lbd_locked: usize,
    first_possibly_free: usize,
    /// Associates each clause id to to a clause.
    /// Unassigned clause ids point to a tautological clause in order to always point to valid one.
    /// This is to avoid having invalid data in this array and resorting to unsafe code to skip validation.
    clauses: RefVec<ClauseId, Clause>,
    /// Metadata for the clause, including its activity and whether it is a learnt clause or not.
    /// A clause id appears in this map if and only if it has been assigned.
    metadata: RefMap<ClauseId, ClauseMetadata>,
    /// A tautological that should be always true in the model.
    /// It is used as a place order for unassigned ids.
    tautological_clause: Clause,
}

impl ClauseDb {
    /// Creates a new database.
    ///
    /// The tautology literal is a literal that is always true in the model and is used to fill placeholder clauses.
    pub fn new(params: ClausesParams) -> ClauseDb {
        ClauseDb {
            params,
            num_fixed: 0,
            num_clauses: 0,
            num_learnt_lbd_locked: 0,
            first_possibly_free: 0,
            clauses: RefVec::new(),
            metadata: RefMap::default(),
            tautological_clause: Clause::new(Disjunction::new(vec![Lit::TRUE])),
        }
    }

    fn is_the_tautological_clause(&self, clause: &Clause) -> bool {
        assert!(self.tautological_clause.unwatched.is_empty());
        clause.watch1 == self.tautological_clause.watch1
            && clause.watch2 == self.tautological_clause.watch2
            && clause.unwatched.is_empty()
    }

    pub fn add_clause(&mut self, cl: Clause, learnt: bool) -> ClauseId {
        self.num_clauses += 1;
        if !learnt {
            self.num_fixed += 1;
        }

        let meta = ClauseMetadata {
            activity: 0f64,
            lbd: 0,
            learnt,
        };

        // too costly to check when the number of clause grows
        // debug_assert!((0..self.first_possibly_free).all(|i| self.is_in_db(ClauseId::from(i))));

        // find a free spot in the database
        let first_free_spot = (self.first_possibly_free..self.clauses.len())
            .map(ClauseId::from)
            .find(|&id| !self.metadata.contains(id));

        // insert in first free spot
        let id = match first_free_spot {
            Some(id) => {
                // we have a free spot, fill it in with the new clause
                debug_assert!(!self.metadata.contains(id));
                debug_assert!(self.is_the_tautological_clause(&self.clauses[id]));
                self.clauses[id] = cl;
                self.metadata.insert(id, meta);
                id
            }
            None => {
                // no free spot, add the clause at the end of the database
                debug_assert_eq!(self.num_clauses - 1, self.clauses.len()); // note: we have already incremented the clause counts
                                                                            // no free spaces push at the end
                let id = self.clauses.push(cl);
                self.metadata.insert(id, meta);
                id
            }
        };
        self.first_possibly_free = usize::from(id) + 1;

        id
    }

    pub fn is_learnt(&self, clause: ClauseId) -> bool {
        self.metadata[clause].learnt
    }

    pub fn num_clauses(&self) -> usize {
        self.num_clauses
    }
    pub fn num_learnt(&self) -> usize {
        self.num_clauses - self.num_fixed
    }

    pub fn num_removable(&self) -> usize {
        if self.num_clauses % 128 == 0 {
            // this is costly check so only do it once in a while, even in debug mode
            debug_assert_eq!(
                self.all_clauses()
                    .filter(|&cl_id| {
                        let meta = self.metadata[cl_id];
                        meta.learnt && meta.lbd != 0 && meta.lbd <= self.params.locked_lbd_level
                    })
                    .count(),
                self.num_learnt_lbd_locked
            );
        }
        self.num_learnt() - self.num_learnt_lbd_locked
    }

    pub fn all_clauses(&self) -> impl Iterator<Item = ClauseId> + '_ {
        #[allow(deprecated)] // ok because we know the table to be dense
        self.metadata.keys()
    }

    /// Set the LBD value of the clause
    pub fn set_lbd(&mut self, clause: ClauseId, lbd: u32) {
        debug_assert_ne!(lbd, 0);
        let meta = &mut self.metadata[clause];
        if meta.learnt {
            // we need to keep track of the number of learnt clauses that are locked
            // for low LBD number
            let lock_level = self.params.locked_lbd_level;
            if meta.lbd == 0 || meta.lbd > lock_level {
                // previously unset or unlocked
                if lbd <= lock_level {
                    // is locked, bump counter
                    self.num_learnt_lbd_locked += 1
                }
            } else if meta.lbd <= lock_level && lbd > lock_level {
                debug_assert_ne!(meta.lbd, 0);
                // the clause will not be locked anymore
                self.num_learnt_lbd_locked -= 1;
            }
        }
        meta.lbd = lbd;
    }

    /// Returns the current LBD value from the clause (updated in unit propagation)
    pub fn get_lbd(&self, clause: ClauseId) -> Option<u32> {
        let lbd = self.metadata[clause].lbd;
        if lbd == 0 {
            None
        } else {
            Some(lbd)
        }
    }

    pub fn bump_activity(&mut self, cl: ClauseId) {
        self.metadata[cl].activity += self.params.cla_inc;
        if self.metadata[cl].activity > 1e100_f64 {
            self.rescale_activities()
        }
    }

    pub fn decay_activities(&mut self) {
        self.params.cla_inc /= self.params.cla_decay;
    }

    fn rescale_activities(&mut self) {
        #[allow(deprecated)] // of because we know the table to be dense
        for meta in self.metadata.values_mut() {
            meta.activity *= 1e-100_f64
        }
        self.params.cla_inc *= 1e-100_f64;
    }

    /// Reduce the size of database by removing half of the clauses that were:
    ///  - learnt, and
    ///  - are not locked, and
    ///  - have a high LBD value
    pub fn reduce_db<F: Fn(ClauseId) -> bool>(&mut self, locked: F, remove_watch: &mut impl FnMut(ClauseId, Lit)) {
        #[allow(deprecated)] // ok because we know the table to be dense
        let mut clauses: Vec<_> = self
            .metadata
            .entries()
            .filter_map(|(id, meta)| {
                if meta.lbd <= self.params.locked_lbd_level {
                    // this clause should be kept because of its low LBD value
                    None
                } else if meta.learnt && !locked(id) {
                    // let score = meta.activity / ((meta.lbd) as f64);
                    let score = meta.activity;
                    Some((id, score))
                } else {
                    None
                }
            })
            .collect();

        clauses.sort_by(|&a, &b| a.1.partial_cmp(&b.1).unwrap_or(Equal));
        // remove half removable
        clauses.iter().take(clauses.len() / 2).for_each(|&(id, _)| {
            let cl = &self.clauses[id];
            if !cl.is_empty() {
                remove_watch(id, !cl.watch1);
            }
            if cl.len() >= 2 {
                remove_watch(id, !cl.watch2);
            }
            self.clauses[id] = self.tautological_clause.clone();
            self.metadata.remove(id);
            self.num_clauses -= 1;
        });

        // make sure we search for free spots from the beginning
        self.first_possibly_free = 0;
    }

    /// Returns true is the clause id is assigned to a clause
    /// Any publicly available clause id should be assigned.
    pub fn is_in_db(&self, clause: ClauseId) -> bool {
        self.metadata.contains(clause)
    }
}

impl Index<ClauseId> for ClauseDb {
    type Output = Clause;
    fn index(&self, k: ClauseId) -> &Self::Output {
        debug_assert!(self.is_in_db(k));
        &self.clauses[k]
    }
}
impl IndexMut<ClauseId> for ClauseDb {
    fn index_mut(&mut self, k: ClauseId) -> &mut Self::Output {
        debug_assert!(self.is_in_db(k));
        &mut self.clauses[k]
    }
}
