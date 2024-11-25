use crate::backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail};
use crate::collections::set::{IterableRefSet, RefSet};
use crate::core::literals::{Disjunction, WatchSet, Watches};
use crate::core::state::{Domains, DomainsSnapshot, Event, Explanation, InferenceCause};
use crate::core::*;
use crate::model::extensions::DisjunctionExt;
use crate::reasoners::sat::clauses::*;
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use itertools::Itertools;
use smallvec::alloc::collections::VecDeque;

/// Keeps track of which clauses are locked.
/// Clauses are locked when used for unit propagation as they must remain available
/// for explanations.
#[derive(Clone)]
struct ClauseLocks {
    locked: RefSet<ClauseId>,
    count: usize,
}
impl ClauseLocks {
    pub fn new() -> Self {
        ClauseLocks {
            locked: Default::default(),
            count: 0,
        }
    }

    pub fn contains(&self, clause: ClauseId) -> bool {
        self.locked.contains(clause)
    }

    pub fn num_locks(&self) -> usize {
        self.count
    }

    pub fn lock(&mut self, clause: ClauseId) {
        debug_assert!(!self.locked.contains(clause));
        self.locked.insert(clause);
        self.count += 1
    }

    pub fn unlock(&mut self, clause: ClauseId) {
        debug_assert!(self.locked.contains(clause));
        self.locked.remove(clause);
        self.count -= 1;
    }
}

#[derive(Clone)]
enum SatEvent {
    Lock(ClauseId),
}

#[derive(Clone)]
pub struct SearchParams {
    /// Given a problem with N clauses, the number of learnt clause will initially be
    ///     init_learnt_base + N * int_learnt_ratio
    init_learnt_ratio: f64,
    init_learnt_base: f64,
    /// Ratio by which we will expand the DB size on an increase
    db_expansion_ratio: f64,
    /// ratio by which we will increase the number of allowed conflict before doing a new DB increase
    increase_ratio_of_conflicts_before_db_expansion: f64,
}
impl Default for SearchParams {
    fn default() -> Self {
        SearchParams {
            init_learnt_ratio: 1_f64 / 3_f64,
            init_learnt_base: 1000_f64,
            db_expansion_ratio: 1.05_f64,
            increase_ratio_of_conflicts_before_db_expansion: 1.5_f64,
        }
    }
}

#[derive(Clone)]
struct SearchState {
    allowed_learnt: f64,
    /// Number of conflicts (as given in stats) at which the last DB expansion was made.
    conflicts_at_last_db_expansion: u64,
    allowed_conflicts_before_db_expansion: u64,
}

impl Default for SearchState {
    fn default() -> Self {
        SearchState {
            allowed_learnt: f64::NAN,
            conflicts_at_last_db_expansion: 0,
            allowed_conflicts_before_db_expansion: 100, // TODO: read from env and synchronize with restarts
        }
    }
}

#[derive(Debug, Clone)]
pub struct Stats {
    pub conflicts: u64,
    pub propagations: u64,
}

#[allow(clippy::derivable_impls)]
impl Default for Stats {
    fn default() -> Self {
        Stats {
            conflicts: 0,
            propagations: 0,
        }
    }
}

/// A clause that has been recorded but not propagated yet.
#[derive(Copy, Clone)]
struct PendingClause {
    /// Id of the clause to propagate
    clause: ClauseId,
}

#[derive(Clone)]
pub struct SatSolver {
    pub clauses: ClauseDb,
    watches: Watches<ClauseId>,
    events_stream: ObsTrailCursor<Event>,
    identity: ReasonerId,
    /// Clauses that have been added to the database but not processed and propagated yet
    pending_clauses: VecDeque<PendingClause>,
    /// Clauses that are locked (can't be remove from the database).
    /// A clause is locked if it asserted a literal and thus might be needed for an explanation
    locks: ClauseLocks,
    /// A list of changes that need to be undone upon backtracking
    trail: Trail<SatEvent>,
    params: SearchParams,
    state: SearchState,
    stats: Stats,
    /// A working data structure to avoid allocations during propagation
    working_watches: WatchSet<ClauseId>,
    /// A local datastructure used to compute LBD (only present here to avoid allocations)
    working_lbd_compute: IterableRefSet<DecLvl>,
}
impl SatSolver {
    pub fn new(identity: ReasonerId) -> SatSolver {
        SatSolver {
            clauses: ClauseDb::new(ClausesParams::default()),
            watches: Watches::default(),
            events_stream: ObsTrailCursor::new(),
            identity,
            pending_clauses: Default::default(),
            locks: ClauseLocks::new(),
            trail: Default::default(),
            params: Default::default(),
            state: Default::default(),
            stats: Default::default(),
            working_watches: Default::default(),
            working_lbd_compute: Default::default(),
        }
    }

    /// Adds a new clause that will be part of the problem definition.
    /// Returns a unique and stable identifier for the clause.
    pub fn add_clause(&mut self, clause: impl Into<Disjunction>) -> ClauseId {
        self.add_clause_impl(Clause::new(clause.into()), false)
    }

    /// Adds a new clause that only needs to be active when the scope literal is true.
    ///
    /// Invariant: All literals in scoped clauses must only be present if the scope literal is true.
    /// This invariant allows scoped clauses to be eagerly propagated even when the scope literal is unknown.
    pub fn add_clause_scoped(&mut self, clause: impl Into<Disjunction>, scope: Lit) -> ClauseId {
        self.add_clause_impl(Clause::new_scoped(clause.into(), scope), false)
    }

    /// Adds a new clause representing `from => to`.
    pub fn add_implication(&mut self, from: Lit, to: Lit) -> ClauseId {
        self.add_clause([!from, to])
    }

    /// Adds a clause that is implied by the other clauses and that the solver is allowed to forget if
    /// it judges that its constraint database is bloated and that this clause is not helpful in resolution.
    pub fn add_forgettable_clause(&mut self, clause: impl Into<Disjunction>) {
        self.add_clause_impl(Clause::new(clause.into()), true);
    }

    /// Adds an asserting clause that was learnt as a result of a conflict
    /// On the next propagation, the clause will be propagated should assert a new literal.
    /// We set it to the front of the propagation queue as we know it will be triggered.
    pub fn add_learnt_clause(&mut self, clause: impl Into<Disjunction>) {
        self.stats.conflicts += 1;
        let clause = clause.into();
        let cl_id = self.clauses.add_clause(Clause::new(clause), true);

        self.pending_clauses.push_front(PendingClause { clause: cl_id });
    }

    fn add_clause_impl(&mut self, clause: Clause, learnt: bool) -> ClauseId {
        let cl_id = self.clauses.add_clause(clause, learnt);
        self.pending_clauses.push_back(PendingClause { clause: cl_id });
        cl_id
    }

    /// Process a newly added clause, making no assumption on the status of the clause.
    ///
    /// The only requirement is that the clause should not have been processed yet.
    fn process_arbitrary_clause(&mut self, cl_id: ClauseId, model: &mut Domains) -> Option<ClauseId> {
        let clause = &self.clauses[cl_id];
        if clause.is_empty() {
            // empty clause is always violated
            return self.process_violated(cl_id, model);
        } else if clause.has_single_literal() {
            let l = clause.watch1;
            self.watches.add_watch(cl_id, !l);
            return match model.value(l) {
                None => {
                    self.set_from_unit_propagation(l, cl_id, model);
                    None
                }
                Some(true) => None,
                Some(false) => self.process_violated(cl_id, model),
            };
        }
        debug_assert!(clause.len() >= 2);

        // clause has at least two literals
        self.move_watches_front(cl_id, model);
        let clause = &self.clauses[cl_id];

        let l0 = clause.watch1;
        let l1 = clause.watch2;

        if model.entails(l0) {
            // satisfied, set watchers and leave state unchanged
            self.set_watch_on_first_literals(cl_id);
            None
        } else if model.entails(!l0) {
            debug_assert!(model.violated_clause(clause));
            // base clause is violated
            self.set_watch_on_first_literals(cl_id);
            self.process_violated(cl_id, model)
        } else if model.value(l1).is_none() && !model.fusable(l0, l1) {
            debug_assert!(!model.unit_clause(clause));
            // pending, set watch and leave state unchanged
            debug_assert!(model.value(l0).is_none());
            debug_assert!(model.pending_clause(clause));
            self.set_watch_on_first_literals(cl_id);
            None
        } else {
            // clause is unit: either a single literal unset or two fusable literals unset
            debug_assert!(model.value(l0).is_none());
            debug_assert!(model.unit_clause(clause));
            self.process_unit_clause(cl_id, model);
            None
        }
    }

    /// Process a clause that is violated. This means that the clause will be deactivated if possible.
    /// Otherwise, it means we are in a conflict state.
    ///
    /// Returns:
    ///  - None, if we are *not* in a conflict (i.e. has been deactivated or was already inactive)
    ///  - Some(cl_id): if we are in a conflict state (the clause could not be deactivated), where
    ///    cl_id is the id of the violated clause passed as parameter.
    #[must_use]
    fn process_violated(&mut self, cl_id: ClauseId, model: &mut Domains) -> Option<ClauseId> {
        debug_assert!(model.violated_clause(&self.clauses[cl_id]));

        // clause is violated, which means we have a conflict
        Some(cl_id)
    }

    /// Selects the two literals that should be watched and places in the `watch1` and `watch2` attributes of the clause.
    fn move_watches_front(&mut self, cl_id: ClauseId, model: &Domains) {
        self.clauses[cl_id].move_watches_front(
            |l| model.value(l),
            |l| {
                debug_assert_eq!(model.value(l), Some(true));
                model.implying_event(l)
            },
            |l| model.presence(l),
        );
    }

    fn process_unit_clause(&mut self, cl_id: ClauseId, model: &mut Domains) {
        let clause = &self.clauses[cl_id];
        debug_assert!(model.unit_clause(clause));

        if clause.has_single_literal() {
            let l = clause.watch1;
            debug_assert!(model.value(l).is_none());
            debug_assert!(!self.watches.is_watched_by(!l, cl_id));
            // watch the only literal
            self.watches.add_watch(cl_id, !l);
            self.set_from_unit_propagation(l, cl_id, model);
        } else {
            debug_assert!(clause.len() >= 2);

            // Set up watch, the first literal must be undefined and the others violated.
            self.move_watches_front(cl_id, model);

            self.set_watch_on_first_literals(cl_id);

            let clause = &mut self.clauses[cl_id];

            if model.fusable(clause.watch1, clause.watch2) {
                // the two watches are fusable, which mean the clause can be propagated if they are both unset
                debug_assert!(model.violated_clause(clause.literals().dropping(2)));
                if clause.watch1 == !model.presence(clause.watch2) {
                    clause.swap_watches()
                }
                let opt = clause.watch1;
                let absent = clause.watch2;
                debug_assert!(absent == !model.presence(opt));
                if model.entails(!opt) {
                    debug_assert!(!model.entails(!absent), "No unset literals in clause...");
                    self.set_from_unit_propagation(absent, cl_id, model);
                } else {
                    // we are in the situation where we have inferred `opt v absent`
                    // where   absent <->  opt = ø
                    //         opt    <->  opt = T v opt = ø
                    // hence we can always propagate opt, because it subsumes absent
                    self.set_from_unit_propagation(opt, cl_id, model);
                }
            } else {
                debug_assert!(model.violated_clause(clause.literals().dropping(1)));
                let l = clause.watch1;
                debug_assert!(model.value(l).is_none());
                self.set_from_unit_propagation(l, cl_id, model);
            }
        }
    }

    pub fn propagate(&mut self, model: &mut Domains) -> Result<(), Explanation> {
        match self.propagate_impl(model) {
            Ok(()) => Ok(()),
            Err(violated) => {
                let clause = &self.clauses[violated];
                debug_assert!(model.violated_clause(clause));

                let mut explanation = Explanation::with_capacity(clause.len());
                for b in clause {
                    explanation.push(!b);
                }

                // bump the activity of the clause
                self.clauses.bump_activity(violated);
                Err(explanation)
            }
        }
    }

    fn propagate_impl(&mut self, model: &mut Domains) -> Result<(), ClauseId> {
        // process all clauses that have been added since last propagation
        while let Some(PendingClause { clause }) = self.pending_clauses.pop_front() {
            if let Some(conflict) = self.process_arbitrary_clause(clause, model) {
                return Err(conflict);
            }
        }
        // grow or shrink database. Placed here to be as close as possible to initial minisat
        // implementation where this appeared in search
        self.scale_database();

        self.propagate_enqueued(model)
    }

    /// Returns:
    ///   `Err(cid)`: in case of a conflict where `cid` is the id of the violated clause
    ///   `Ok(())` if no conflict was detected during propagation
    fn propagate_enqueued(&mut self, model: &mut Domains) -> Result<(), ClauseId> {
        debug_assert!(
            self.pending_clauses.is_empty(),
            "Some clauses have not been integrated in the database yet."
        );

        // take ownership of the working data structure, replace it by an empty one
        // (this does not require any allocation)
        let mut working_watches = WatchSet::new();
        std::mem::swap(&mut self.working_watches, &mut working_watches);

        while let Some(&ev) = self.events_stream.pop(model.trail()) {
            let new_lit = ev.new_literal();

            // remove all watches and place them on our local copy
            working_watches.clear();
            self.watches.move_watches_to(new_lit, &mut working_watches);
            debug_assert_eq!(
                working_watches.watches_on(new_lit).count(),
                working_watches.all_watches().count()
            );
            // will be set to `Some(cid)` if propagation encounters a contradicting clause `cid`
            let mut contradicting_clause = None;

            for watch in working_watches.all_watches() {
                let watched_literal = watch.to_lit(new_lit.svar());
                let clause = watch.watcher;
                debug_assert!(self.clauses[clause].literals().any(|l| l == !watched_literal));

                // we propagate unless:
                // - we found a contradicting clause earlier
                // - the event does not makes the watched literal true (meaning it was already true before this event)
                if contradicting_clause.is_none() && ev.makes_true(watched_literal) {
                    if !self.propagate_clause(clause, new_lit, model) {
                        // propagation found a contradiction
                        contradicting_clause = Some(clause);
                    }
                } else {
                    // we encountered a contradicting clause or the event is not triggering,
                    // we need to restore the watch
                    let to_restore = watch.to_lit(new_lit.svar());
                    self.watches.add_watch(clause, to_restore);
                }
            }

            if let Some(violated) = contradicting_clause {
                // give up ownership of the working data structure
                std::mem::swap(&mut self.working_watches, &mut working_watches);
                return Err(violated);
            }
        }
        // give up ownership of the working data structure
        std::mem::swap(&mut self.working_watches, &mut working_watches);

        Ok(())
    }

    /// Propagate a clause that is watching literal `p` became true.
    /// `p` should be one of the literals watched by the clause.
    /// If the clause is:
    /// - pending: reset another watch and return true
    /// - unit: reset watch, enqueue the implied literal and return true
    /// - violated: reset watch and return false
    fn propagate_clause(&mut self, clause_id: ClauseId, p: Lit, model: &mut Domains) -> bool {
        debug_assert_eq!(model.value(p), Some(true));
        // counter intuitive: this method is only called after removing the watch
        // and we are responsible for resetting a valid watch.
        debug_assert!(!self.watches.is_watched_by(p, clause_id));
        // self.stats.propagations += 1;
        let clause = &mut self.clauses[clause_id];
        if clause.has_single_literal() {
            debug_assert!(p.entails(!clause.watch1));
            // only one literal that is false, the clause is violated
            self.watches.add_watch(clause_id, p);
            return self.process_violated(clause_id, model).is_none();
        }
        if p.entails(!clause.watch1) {
            clause.swap_watches();
        }
        debug_assert!(p.entails(!clause.watch2)); // lits[1] == !p in SAT

        if model.entails(clause.watch1) {
            // clause satisfied, restore the watch and exit
            self.watches.add_watch(clause_id, !clause.watch2);
            return true;
        }

        // we are looking for replacement for watch2, i.e. an unwatched literal that is currently unset

        // a special case in the presence of optional variables is when there are only two unset literals
        // l1 and l2  where  l2 <-> !present(l1)
        //  Recall that a literal   `l1` is a shorthand for  `l1 = T v l1 = ø`
        //  thus if - we have inferred  `l1 v l2` (which is the case if all other literals are false), and
        //     `    - we know that `l2 <-> l1 = ø`
        //  then we can propagate `l1` which is logically equivalent to `l1 v l2`

        enum Replacement {
            /// The literal is not set AND can be fused with `watch1` and these are the only two unset literals
            FusableUnit(usize),
            /// The literal is not set AND can NOT be fused with `watch1`
            Regular(usize),
            /// No other unset variables
            None,
        }

        let mut replacement = Replacement::None;
        for i in 0..clause.unwatched.len() {
            let lit = clause.unwatched[i];
            if !model.entails(!lit) {
                // this is a candidate, distinguish the case where it is fusable with watch1
                if model.fusable(clause.watch1, lit) {
                    if let Replacement::FusableUnit(prev_i) = replacement {
                        // more than one fusable
                        // this might occur for a clause `!p v a v b`   where `p` is the presence of both `a` and `b`
                        // here, !p might be fused with both a and b
                        // the clause is not unit so we should set up a regular watch
                        debug_assert_eq!(!clause.watch1, model.presence(clause.unwatched[prev_i]));
                        debug_assert_eq!(!clause.watch1, model.presence(clause.unwatched[i]));
                        replacement = Replacement::Regular(prev_i);
                        break;
                    } else {
                        debug_assert!(matches!(replacement, Replacement::None));
                        // record that we have found a fusable, but keep searching for other unset literal.
                        // we only want to settle on FusableUnit if there are no other unset literals
                        replacement = Replacement::FusableUnit(i);
                    }
                } else {
                    // not fusable, clause cannot be unit, so select as replacement
                    replacement = Replacement::Regular(i);
                    break;
                }
            }
        }

        match replacement {
            Replacement::Regular(i) => {
                // clause is not unit, we have a replacement, set the watch and exit
                let lit = clause.unwatched[i];
                clause.set_watch2(i);
                self.watches.add_watch(clause_id, !lit);
                true
            }
            Replacement::FusableUnit(i) => {
                // clause is unit because the two only unset literals can be fused
                let lit = clause.unwatched[i];
                clause.set_watch2(i);
                self.watches.add_watch(clause_id, !lit);

                debug_assert!(model.fusable(clause.watch1, clause.watch2));
                // all other literals should be false
                debug_assert!(clause.unwatched.iter().all(|&l| model.entails(!l)));

                // distinguish between the optional literal and its absent literal
                let (opt, absent) = if clause.watch1 == !model.presence(clause.watch2) {
                    (clause.watch2, clause.watch1)
                } else {
                    debug_assert!(clause.watch2 == !model.presence(clause.watch1));
                    (clause.watch1, clause.watch2)
                };

                match (model.value(opt), model.value(absent)) {
                    (Some(true), _) | (_, Some(true)) => true, // clause holds
                    (Some(false), Some(false)) => self.process_violated(clause_id, model).is_none(),
                    (None, None) | (None, Some(false)) => {
                        // we infered `opt v absent`, propagate `opt` which is logically equivalent
                        self.set_from_unit_propagation(opt, clause_id, model);
                        true
                    }
                    (Some(false), None) => {
                        // `absent` is the only unset literal, propagate it
                        self.set_from_unit_propagation(absent, clause_id, model);
                        true
                    }
                }
            }
            Replacement::None => {
                // no replacement found, clause is unit, restore watch and propagate
                self.watches.add_watch(clause_id, !clause.watch2);
                let first_lit = clause.watch1;
                match model.value(first_lit) {
                    Some(true) => true, // clause is true
                    Some(false) => {
                        // clause is violated, deactivate it if possible
                        self.process_violated(clause_id, model).is_none()
                    }
                    None => {
                        self.set_from_unit_propagation(first_lit, clause_id, model);
                        true
                    }
                }
            }
        }
    }

    fn set_from_unit_propagation(&mut self, literal: Lit, propagating_clause: ClauseId, model: &mut Domains) {
        // Set the literal to false.
        // We know that no inconsistency will occur (from the invariants of unit propagation.
        // However, it might be the case that nothing happens if the literal is already known to be absent.
        let changed_something = model.set(literal, self.identity.cause(propagating_clause)).unwrap();
        if changed_something {
            // lock clause to ensure it will not be removed. This is necessary as we might need it to provide an explanation
            self.lock(propagating_clause);
            self.stats.propagations += 1;
            if self.clauses.is_learnt(propagating_clause) {
                let lbd = self.lbd(literal, propagating_clause, model);
                self.clauses.set_lbd(propagating_clause, lbd);
            }
        }
    }

    fn lbd(&mut self, asserted_literal: Lit, clause: ClauseId, model: &Domains) -> u32 {
        let clause = &self.clauses[clause];

        self.working_lbd_compute.clear();

        for l in clause.literals() {
            if l != asserted_literal {
                if !model.entails(!l) {
                    // strange case that may occur due to optionals
                    let lvl = self.current_decision_level() + 1; // future
                    self.working_lbd_compute.insert(lvl);
                } else {
                    let lvl = model.entailing_level(!l);
                    if lvl != DecLvl::ROOT {
                        self.working_lbd_compute.insert(lvl);
                    }
                }
            }
        }
        // returns the number of decision levels, and add one to account for the asserted literal
        self.working_lbd_compute.len() as u32 + 1
    }

    fn lock(&mut self, clause: ClauseId) {
        self.locks.lock(clause);
        self.trail.push(SatEvent::Lock(clause));
    }

    /// set the watch on the first two literals of the clause (without any check)
    /// One should typically call `move_watches_front` on the clause before hand.
    fn set_watch_on_first_literals(&mut self, cl_id: ClauseId) {
        let cl = &self.clauses[cl_id];
        debug_assert!(cl.len() >= 2);
        self.watches.add_watch(cl_id, !cl.watch1);
        self.watches.add_watch(cl_id, !cl.watch2);
    }

    #[allow(dead_code)]
    fn assert_watches_valid(&self, cl_id: ClauseId, state: &Domains) -> bool {
        let cl = &self.clauses[cl_id];
        let l0 = cl.watch1;
        let l1 = cl.watch2;
        // assert!(self.watches[!l0].contains(&cl_id));
        // assert!(self.watches[!l1].contains(&cl_id));
        match state.value_of_clause(cl) {
            Some(true) => {
                // one of the two watches should be entailed
                assert!(state.entails(l0) || state.entails(l1))
            }
            Some(false) => {}
            None => {
                // both watches should be undefined. If only one was undef, then the clause should have replaced the other watch
                // it with an undefined literal, or do unit propagation which should have made the clause true
                assert!(state.value(l0).is_none() && state.value(l1).is_none())
            }
        }
        if state.fusable(l0, l1) {
            assert!(cl.unwatched.iter().all(|&l| state.entails(!l)));
        }
        true
    }

    pub fn explain(&mut self, explained: Lit, cause: u32, model: &DomainsSnapshot, explanation: &mut Explanation) {
        let explained_presence = model.presence(explained);
        let clause = ClauseId::from(cause);

        // bump the activity of any clause used in an explanation
        self.clauses.bump_activity(clause);
        let clause = &self.clauses[clause];

        // we have a clause  A1 v A2 v ... v An v EXPL
        debug_assert!(clause.literals().any(|l| l.entails(explained)));
        // we must provide an explanation of the form
        // !A1 & !A2 ... -> EXPL
        // if EXPL is optional, EXPL is equivalent to (EXPL=false v !PREZ(EXPL)
        // we may omit PREZ(EXPL) from the explanation as it is already implictly accounted for in the inference consequence

        explanation.reserve(clause.len() - 1);
        for l in clause {
            if l.entails(explained) {
                // the explained literal, omit
            } else if l != !explained_presence {
                // add to explanation unless it represents the absence of the explained literal
                explanation.push(!l);
            }
        }
    }

    /// Function responsible for scaling the size of the clause Database.
    /// The database has a limited number of slots for learnt clauses.
    /// If all slots are taken, this function can:
    ///  - expand the database with more slots. This occurs if a certain number of conflicts occurred
    ///    since the last expansion.
    ///  - Remove learnt clauses from the DB. This typically removes about half the clauses, making
    ///    sure that clauses that are used to explain the current value of the literal at kept.
    ///    Clauses to be removed are the least active ones.
    fn scale_database(&mut self) {
        if self.state.allowed_learnt.is_nan() {
            let initial_clauses = self.clauses.num_clauses() - self.clauses.num_learnt();
            self.state.allowed_learnt =
                self.params.init_learnt_base + initial_clauses as f64 * self.params.init_learnt_ratio;
        }
        if self.clauses.num_removable() as i64 - self.locks.num_locks() as i64 >= self.state.allowed_learnt as i64 {
            // we exceed the number of learnt clause in the DB.
            // Check if it is time to increase the DB maximum size, otherwise shrink it.
            if self.stats.conflicts - self.state.conflicts_at_last_db_expansion
                >= self.state.allowed_conflicts_before_db_expansion
            {
                // increase the number of allowed learnt clause in the database
                self.state.allowed_learnt *= self.params.db_expansion_ratio;

                // record the number of conflict at this db expansion
                self.state.conflicts_at_last_db_expansion = self.stats.conflicts;
                // increase the number of conflicts allowed before the next expansion
                self.state.allowed_conflicts_before_db_expansion =
                    (self.state.allowed_conflicts_before_db_expansion as f64
                        * self.params.increase_ratio_of_conflicts_before_db_expansion) as u64;
            } else {
                // reduce the database size
                let locks = &self.locks;
                let watches = &mut self.watches;
                let mut remove_watch = |clause: ClauseId, watched: Lit| {
                    watches.remove_watch(clause, watched);
                };
                self.clauses.reduce_db(|cl| locks.contains(cl), &mut remove_watch);
            }
        }
    }

    pub fn print_stats(&self) {
        println!("DB size              : {}", self.clauses.num_clauses());
        println!("Num unit propagations: {}", self.stats.propagations);
    }
}

impl Backtrack for SatSolver {
    fn save_state(&mut self) -> DecLvl {
        self.trail.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }

    fn restore_last(&mut self) {
        let locks = &mut self.locks;
        self.trail.restore_last_with(|SatEvent::Lock(cl)| locks.unlock(cl));
    }
}

impl Theory for SatSolver {
    fn identity(&self) -> ReasonerId {
        self.identity
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        Ok(self.propagate(model)?)
    }

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        self.explain(literal, context.payload, model, out_explanation)
    }

    fn print_stats(&self) {
        self.print_stats()
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtrack::Backtrack;
    use crate::collections::seq::Seq;
    use crate::core::state::{Cause, Explainer, InferenceCause};
    use crate::model::extensions::AssignmentExt;

    type Model = crate::model::Model<&'static str>;

    #[test]
    fn test_propagation_simple() {
        let writer = ReasonerId::Sat;
        let model = &mut Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");

        let mut sat = SatSolver::new(writer);
        let a_or_b = vec![a.true_lit(), b.true_lit()];

        sat.add_clause(a_or_b);
        sat.propagate(&mut model.state).unwrap();
        assert_eq!(model.boolean_value_of(a), None);
        assert_eq!(model.boolean_value_of(b), None);
        model.state.set(a.false_lit(), Cause::Decision).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), None);
        sat.propagate(&mut model.state).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), Some(true));
    }

    #[test]
    fn test_propagation_complex() {
        let writer = ReasonerId::Sat;
        let model = &mut Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");
        let c = model.new_bvar("c");
        let d = model.new_bvar("d");

        let check_values = |model: &Model, values: [Option<bool>; 4]| {
            assert_eq!(model.boolean_value_of(a), values[0]);
            assert_eq!(model.boolean_value_of(b), values[1]);
            assert_eq!(model.boolean_value_of(c), values[2]);
            assert_eq!(model.boolean_value_of(d), values[3]);
        };
        check_values(model, [None, None, None, None]);

        let mut sat = SatSolver::new(writer);
        let clause = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.true_lit()];

        sat.add_clause(clause);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [None, None, None, None]);

        model.save_state();
        model.state.decide(a.false_lit()).unwrap();
        check_values(model, [Some(false), None, None, None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), None, None, None]);

        model.save_state();
        model.state.decide(b.false_lit()).unwrap();
        check_values(model, [Some(false), Some(false), None, None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), None, None]);

        model.save_state();
        model.state.decide(c.true_lit()).unwrap();
        check_values(model, [Some(false), Some(false), Some(true), None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), Some(true), None]);

        model.save_state();
        model.state.decide(d.false_lit()).unwrap();
        check_values(model, [Some(false), Some(false), Some(true), Some(false)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), Some(true), Some(false)]);

        model.restore_last();
        check_values(model, [Some(false), Some(false), Some(true), None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), Some(true), None]);

        model.restore_last();
        check_values(model, [Some(false), Some(false), None, None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), None, None]);

        model.state.decide(c.false_lit()).unwrap();
        check_values(model, [Some(false), Some(false), Some(false), None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), Some(false), Some(true)]);
    }

    #[test]
    fn test_propagation_failure() {
        let writer = ReasonerId::Sat;
        let model = &mut Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");

        let mut sat = SatSolver::new(writer);
        let a_or_b = vec![a.true_lit(), b.true_lit()];

        sat.add_clause(a_or_b);
        sat.propagate(&mut model.state).unwrap();
        assert_eq!(model.boolean_value_of(a), None);
        assert_eq!(model.boolean_value_of(b), None);
        model.state.set(a.false_lit(), Cause::Decision).unwrap();
        model.state.set(b.false_lit(), Cause::Decision).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), Some(false));
        assert!(sat.propagate(&mut model.state).is_err());
    }

    #[test]
    fn test_online_clause_insertion() {
        let writer = ReasonerId::Sat;
        let model = &mut Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");
        let c = model.new_bvar("c");
        let d = model.new_bvar("d");

        let mut sat = SatSolver::new(writer);

        let check_values = |model: &Model, values: [Option<bool>; 4]| {
            assert_eq!(model.boolean_value_of(a), values[0], "a");
            assert_eq!(model.boolean_value_of(b), values[1], "b");
            assert_eq!(model.boolean_value_of(c), values[2], "c");
            assert_eq!(model.boolean_value_of(d), values[3], "d");
        };
        check_values(model, [None, None, None, None]);

        // not(a) and not(b)
        model.state.set(a.false_lit(), Cause::Decision).unwrap();
        model.state.set(b.false_lit(), Cause::Decision).unwrap();
        check_values(model, [Some(false), Some(false), None, None]);

        let abcd = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.true_lit()];
        sat.add_clause(abcd);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), None, None]);

        let nota_notb = vec![a.false_lit(), b.false_lit()];
        sat.add_clause(nota_notb);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), None, None]);

        let nota_b = vec![a.false_lit(), b.true_lit()];
        sat.add_clause(nota_b);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [Some(false), Some(false), None, None]);

        let a_b_notc = vec![a.true_lit(), b.true_lit(), c.false_lit()];
        sat.add_clause(a_b_notc);
        sat.propagate(&mut model.state).unwrap(); // should trigger and in turn trigger the first clause
        check_values(model, [Some(false), Some(false), Some(false), Some(true)]);

        let violated = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.false_lit()];
        sat.add_clause(violated);
        assert!(sat.propagate(&mut model.state).is_err());
    }

    #[test]
    fn test_int_propagation() {
        let writer = ReasonerId::Sat;
        let model = &mut Model::new();
        let a = model.new_ivar(0, 10, "a");
        let b = model.new_ivar(0, 10, "b");
        let c = model.new_ivar(0, 10, "c");
        let d = model.new_ivar(0, 10, "d");

        let check_values = |model: &Model, values: [(IntCst, IntCst); 4]| {
            assert_eq!(model.domain_of(a), values[0]);
            assert_eq!(model.domain_of(b), values[1]);
            assert_eq!(model.domain_of(c), values[2]);
            assert_eq!(model.domain_of(d), values[3]);
        };
        check_values(model, [(0, 10), (0, 10), (0, 10), (0, 10)]);

        let mut sat = SatSolver::new(writer);
        let clause = vec![Lit::leq(a, 5), Lit::leq(b, 5)];
        sat.add_clause(clause);
        let clause = vec![Lit::geq(c, 5), Lit::geq(d, 5)];
        sat.add_clause(clause);

        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(0, 10), (0, 10), (0, 10), (0, 10)]);

        // lower bound changes

        model.state.set_lb(a, 4, Cause::Decision).unwrap();
        check_values(model, [(4, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(4, 10), (0, 10), (0, 10), (0, 10)]);

        model.state.set_lb(a, 5, Cause::Decision).unwrap();
        check_values(model, [(5, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(5, 10), (0, 10), (0, 10), (0, 10)]);

        // trigger first clause
        model.save_state();
        sat.save_state();
        model.state.set_lb(a, 6, Cause::Decision).unwrap();
        check_values(model, [(6, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(6, 10), (0, 5), (0, 10), (0, 10)]);

        // retrigger first clause with stronger literal
        model.restore_last();
        sat.restore_last();
        check_values(model, [(5, 10), (0, 10), (0, 10), (0, 10)]);
        model.state.set_lb(a, 8, Cause::Decision).unwrap();
        check_values(model, [(8, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 10), (0, 10)]);

        // Upper bound changes

        model.state.set_ub(c, 6, Cause::Decision).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 6), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 6), (0, 10)]);

        model.state.set_ub(c, 5, Cause::Decision).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 5), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 5), (0, 10)]);

        // should trigger second clause
        model.save_state();
        sat.save_state();
        model.state.set_ub(c, 4, Cause::Decision).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 4), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 4), (5, 10)]);

        // retrigger second clause with stronger literal
        model.restore_last();
        sat.restore_last();
        check_values(model, [(8, 10), (0, 5), (0, 5), (0, 10)]);
        model.state.set_ub(c, 2, Cause::Decision).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 2), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(model, [(8, 10), (0, 5), (0, 2), (5, 10)]);
    }

    #[test]
    fn test_clauses_with_optionals() {
        let m = &mut Model::new();
        struct Exp<'a> {
            sat: &'a mut SatSolver,
        }
        impl<'a> Explainer for Exp<'a> {
            fn explain(
                &mut self,
                cause: InferenceCause,
                literal: Lit,
                model: &DomainsSnapshot,
                explanation: &mut Explanation,
            ) {
                self.sat.explain(literal, cause.payload, model, explanation);
            }
        }
        fn check_explanation(m: &Model, sat: &mut SatSolver, lit: Lit, expected: impl Seq<Lit>) {
            let result = m.state.implying_literals(lit, &mut Exp { sat }).unwrap();
            assert_eq!(result.to_set(), expected.to_set());
        }

        let px = m.new_presence_variable(Lit::TRUE, "px").true_lit();
        let x1 = m.new_optional_bvar(px, "x1").true_lit();
        let x2 = m.new_optional_bvar(px, "x2").true_lit();

        let py = m.new_presence_variable(Lit::TRUE, "py").true_lit();
        let y1 = m.new_optional_bvar(py, "y1").true_lit();
        let y2 = m.new_optional_bvar(py, "y2").true_lit();

        let pz = m.get_conjunctive_scope(&[px, py]);
        let z1 = m.new_optional_bvar(pz, "z1").true_lit();
        let z2 = m.new_optional_bvar(pz, "z2").true_lit();

        let sat = &mut SatSolver::new(ReasonerId::Sat);

        m.save_state();
        sat.save_state();

        sat.add_clause_scoped([x1, x2], px);

        m.state.decide(!x1).unwrap();
        sat.propagate(&mut m.state).unwrap();
        assert!(m.entails(x2));
        assert!(m.value_of_literal(px).is_none());
        check_explanation(m, sat, x2, [!x1]);

        assert!(!m.entails(!py));
        sat.add_clause_scoped([!y2, y1], py);
        sat.add_clause_scoped([!y2, !y1], py);
        m.state.decide(y2).unwrap();
        sat.propagate(&mut m.state).unwrap();
        assert!(m.entails(!py));
        check_explanation(m, sat, !py, [y2, y1]); // note: could be be !y1 as well depending on propagation order.

        m.reset();
        m.save_state();
        sat.reset();
        sat.save_state();

        assert!(!m.entails(!py));
        sat.add_clause_scoped([y1, y2], py);
        m.state.decide(!y1).unwrap();
        m.state.decide(!y2).unwrap();
        sat.propagate(&mut m.state).unwrap();
        assert!(m.entails(!py));
        check_explanation(m, sat, !py, [!y1, !y2]);

        m.reset();
        m.save_state();
        sat.reset();
        sat.save_state();

        assert!(!m.entails(!pz));
        sat.add_clause_scoped([z1, z2], pz);
        m.state.decide(pz).unwrap();
        m.state.decide(!z1).unwrap();
        m.state.decide(!z2).unwrap();

        assert!(sat.propagate(&mut m.state).is_err());
    }
}
