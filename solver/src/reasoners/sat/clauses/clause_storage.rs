//! Module that provides a way to create a clause [`Clause`] and arrays of clause [`Clauses`].
//! The lifetime of each clause is always tied to the lifetime to the array in which it was created.

use std::{
    fmt::{Debug, Display, Error, Formatter},
    ops::{Index, IndexMut},
};

use bumpalo::Bump;
use itertools::Itertools;

use crate::{
    backtrack::EventIndex,
    collections::ref_store::RefVec,
    core::{
        literals::{Disjunction, Lits},
        Lit,
    },
    reasoners::sat::clauses::ClauseId,
};

/// Arena allocator type
type Arena = Bump<8>;

/// Provide storage for clauses that associates each [`Clause`] to a [`ClauseId`].
/// Clauses are allocated into an internal allocation arena (bump allocator) which allows for efficient individual allocation and batched deallocation.
///
/// It can be extended with new clauses but we do not provide methods for removing a single clause (because it cannot be individually deallocated).
/// The method [`clone_with_subset`] however provides a way to
///
///
/// # Safety
///
/// The module exposes a safe API but uses some unsafe operations internally.
pub struct Clauses {
    /// Array of clauses, indexed by their ID.
    ///
    /// The array is dense and all `ClauseId` is associated to a valid clause, with a tautological clause used a placeholder.
    ///
    /// # Safety
    ///
    /// The litetime of the clauses in the array is tied to the allocation arena present in the same struct.
    /// Hence neither the array nor an individual clause should ever be leaked without a lifetime that encompasses the arena's lifetime as well.
    clauses: RefVec<ClauseId, Clause>,
    buffer: Lits,

    /// Arena allocator, from which the clause borrow memory
    ///
    /// # Safety
    /// This should *always* be dropped after any of the clauses that used it for allocation.
    /// This is enforced by its location as the last field, which means it will be dropped last.
    ///
    arena: Arena,
}

impl Clauses {
    pub fn new() -> Self {
        Self {
            clauses: Default::default(),
            buffer: Lits::with_capacity(64),
            arena: Arena::with_min_align(),
        }
    }
    pub fn len(&self) -> usize {
        self.clauses.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, clause_id: ClauseId) -> &Clause {
        &self.clauses[clause_id]
    }

    /// Creates a new clause with the given set of literals.
    ///
    /// UNSAFE because, the lifetime of the returned clauses is not properly captured by the type system but is
    /// instead tied to the lifetime of the arena in `self`.
    unsafe fn create_clause(&mut self, clause: &[Lit], scope: Option<Lit>) -> Clause {
        // use the internal buffer to pre-process the clause
        self.buffer.clear();
        self.buffer.extend_from_slice(clause);
        if let Some(scope) = scope {
            // has a scope of the form `scope -> Or(lits)`
            // merge it in the clause
            self.buffer.push(!scope);
        }
        self.buffer.simplify_disjunctive();
        // create and return the new clause from the pre-processed literals
        Clause::new(&self.buffer, &self.arena)
    }

    pub fn push(&mut self, clause: &[Lit], scope: Option<Lit>) -> ClauseId {
        unsafe {
            // safe because the clause is immediately stored inside the array, whose lifetime is tied to the one of the arena
            let clause = self.create_clause(clause, scope);
            self.clauses.push(clause)
        }
    }
    pub fn set(&mut self, clause_id: ClauseId, clause: &[Lit], scope: Option<Lit>) {
        unsafe {
            // safe because the clause is immediately stored inside the array, whose lifetime is tied to the one of the arena
            let clause = self.create_clause(clause, scope);
            self.clauses[clause_id] = clause;
        }
    }

    /// Creates a clone of the clauses (with stable identifiers, keeping only the clauses for which `retain` return true).
    /// Deleted clauses will be replaced by a tautological clause and can be safely overritten.
    pub(super) fn clone_with_subset(&self, retain: impl Fn(ClauseId) -> bool) -> Self {
        let mut copy = Clauses::new();
        for (cid, cl) in self.clauses.entries() {
            unsafe {
                let cl = if retain(cid) {
                    Clause {
                        watch1: cl.watch1,
                        watch2: cl.watch2,
                        // Safety: erase the lifetime of the copy's arena !!
                        unwatched: std::mem::transmute::<&mut [Lit], &'static mut [Lit]>(
                            copy.arena.alloc_slice_copy(cl.unwatched_lits()),
                        ),
                    }
                } else {
                    Clause::tautology()
                };
                // safety: push the clause to the copy, thus tying its lifetime to the one of the arena
                let new_cid = copy.clauses.push(cl);
                debug_assert!(cid == new_cid);
            }
        }
        copy
    }
}

impl Default for Clauses {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Clauses {
    fn clone(&self) -> Self {
        self.clone_with_subset(|_| true)
    }
}

impl Index<ClauseId> for Clauses {
    type Output = Clause;
    fn index(&self, k: ClauseId) -> &Self::Output {
        &self.clauses[k]
    }
}
impl IndexMut<ClauseId> for Clauses {
    fn index_mut(&mut self, k: ClauseId) -> &mut Self::Output {
        &mut self.clauses[k]
    }
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
///
/// # Safety
///
/// The Clause has a hidden lifetime that ties it to the arena. The API provided is made safe by ensuring that the clause
/// (and in particular its internal slice of unwatched literals) by always sharing the `Clause` behind a reference whose lifetime
/// encompasses the one of the allocation. This also implies that it is impossible to get an own version of the `Clause`
/// because the lifetime associated with the reference would be lost.
///
/// ```compile_fail,E0308
/// use aries::reasoners::sat::clauses::Clause;
/// let cl: &Clause = todo!();
/// let copied: Clause = cl.clone(); // should never compile
/// ```
pub struct Clause {
    pub watch1: Lit,
    pub watch2: Lit,

    /// Other literals of the clause, that are not watched
    ///
    /// # Safety
    ///
    /// The 'static lifetime is NOT the real one and the field must remain private.
    ///
    /// The allocated slice comes from the allocation arena passed to the [`new`] method.
    /// It is thus crucial that the clause and this reference never outlives the arena.
    /// THis is enforce by ensuring that:
    ///  - no owned `Clause` can be created outside or leave this module (always accessed by reference)
    ///  - the `Clause` is neither Copy nor Clone (cannot create an owned one from a reference)
    ///  - making sure all clauses are dropped before the arean (drop order in `Clauses`)
    unwatched: &'static mut [Lit],
}
impl Clause {
    /// A tautological clause that should be always true in the model.
    /// It is used as a place order for unassigned ids.
    pub const fn tautology() -> Clause {
        Clause {
            watch1: Lit::TRUE,
            watch2: Lit::TRUE,
            unwatched: &mut [],
        }
    }

    /// Creates a new clause from the disjunctive set of literals.
    /// It is assumed that the set of literals is non empty.
    /// No clean up of the clause will be made to remove redundant literals.
    ///
    /// # Safety
    ///
    /// This is unsafe because the returned Self has a reference to the `alloc` allocation arena which is not captured in its type.
    /// Caller should ensure that the returned object *never* outlives the allocation arena.
    unsafe fn new(lits: &[Lit], alloc: &Arena) -> Self {
        debug_assert!(Disjunction::is_simplified(lits));

        match lits.len() {
            0 => unreachable!("Should have been caught higher-up in the stack"),
            1 => Clause {
                watch1: lits[0],
                watch2: lits[0],
                unwatched: &mut [],
            },
            _ => {
                debug_assert_ne!(lits[0], lits[1]);
                Clause {
                    watch1: lits[0],
                    watch2: lits[1],
                    unwatched: unsafe {
                        // We erase the lifetie of the reference which is tied to the arena.
                        // This is safe because the clause will always be shared behind a reference whose lifetime encompasses the lifetime of the arena
                        std::mem::transmute::<&mut [Lit], &'static mut [Lit]>(alloc.alloc_slice_copy(&lits[2..]))
                    },
                }
            }
        }
    }

    /// Returns the i-th unwatched literal of the clause.
    pub fn unwatched(&self, index: usize) -> Lit {
        // SAFETY: we extend the lifetime of the clause (tied to the oen of the arena)
        // to the lifetime of the internal structure
        self.unwatched[index]
    }

    /// Returns the unwatched literals of the clause
    pub fn unwatched_lits(&self) -> &[Lit] {
        // SAFETY: we extend the lifetime of the clause (tied to the one of the arena)
        // to the lifetime of the internal structure
        self.unwatched
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
