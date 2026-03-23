mod clause_storage;
use crate::collections::ref_store::RefMap;
use crate::core::Lit;
use crate::create_ref_type;
use env_param::EnvParam;
use std::cmp::Ordering::Equal;
use std::collections::BTreeSet;
use std::fmt::{Display, Error, Formatter};
use std::ops::{Index, IndexMut};

pub use clause_storage::*;

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
    /// Metadata for the clause, including its activity and whether it is a learnt clause or not.
    /// A clause id appears in this map if and only if it has been assigned.
    metadata: RefMap<ClauseId, ClauseMetadata>,
    clauses: Clauses,
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
            clauses: Clauses::new(),
            metadata: RefMap::default(),
        }
    }

    fn is_the_tautological_clause(&self, clause: &Clause) -> bool {
        clause.watch1 == Clause::tautology().watch1
            && clause.watch2 == Clause::tautology().watch2
            && clause.unwatched_lits().is_empty()
    }

    pub fn add_clause(&mut self, cl: &[Lit], scope: Option<Lit>, learnt: bool) -> ClauseId {
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
                self.clauses.set(id, cl, scope);
                self.metadata.insert(id, meta);
                id
            }
            None => {
                // no free spot, add the clause at the end of the database
                debug_assert_eq!(self.num_clauses - 1, self.clauses.len()); // note: we have already incremented the clause counts
                // no free spaces push at the end
                let id = self.clauses.push(cl, scope);
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
        if self.num_clauses.is_multiple_of(128) {
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
        if lbd == 0 { None } else { Some(lbd) }
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

        // select first half of the clauses for removal
        let to_remove = BTreeSet::from_iter(clauses.iter().take(clauses.len() / 2).map(|(cid, _)| *cid));
        to_remove.iter().for_each(|&id| {
            let cl = &self.clauses[id];
            if !cl.is_empty() {
                remove_watch(id, !cl.watch1);
            }
            if cl.len() >= 2 {
                remove_watch(id, !cl.watch2);
            }
            self.metadata.remove(id);
            self.num_clauses -= 1;
        });
        // replace the clauses with a new one without the removed clauses
        self.clauses = self.clauses.clone_with_subset(|cid| !to_remove.contains(&cid));

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
