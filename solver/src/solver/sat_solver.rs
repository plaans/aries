use crate::clauses::{Clause, ClauseDb, ClauseId, ClausesParams};
use crate::solver::BindingResult;
use crate::BindSplit;
use aries_backtrack::{Backtrack, DecLvl, ObsTrailCursor, Trail};
use aries_collections::set::RefSet;
use aries_model::bounds::{Disjunction, Lit, WatchSet, Watches};
use aries_model::extensions::DisjunctionExt;
use aries_model::lang::*;
use aries_model::state::{Domains, Event, Explanation};
use aries_model::{Model, WriterId};
use itertools::Itertools;
use smallvec::alloc::collections::VecDeque;
use std::convert::TryFrom;

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
            db_expansion_ratio: 1.1_f64,
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
impl Default for Stats {
    fn default() -> Self {
        Stats {
            conflicts: 0,
            propagations: 0,
        }
    }
}

#[derive(Clone)]
pub struct SatSolver {
    clauses: ClauseDb,
    watches: Watches<ClauseId>,
    events_stream: ObsTrailCursor<Event>,
    token: WriterId,
    /// Clauses that have been added to the database but not processed and propagated yet
    pending_clauses: VecDeque<ClauseId>,
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
}
impl SatSolver {
    pub fn new(token: WriterId) -> SatSolver {
        SatSolver {
            clauses: ClauseDb::new(ClausesParams::default()),
            watches: Watches::default(),
            events_stream: ObsTrailCursor::new(),
            token,
            pending_clauses: Default::default(),
            locks: ClauseLocks::new(),
            trail: Default::default(),
            params: Default::default(),
            state: Default::default(),
            stats: Default::default(),
            working_watches: Default::default(),
        }
    }

    /// Adds a new clause that will be part of the problem definition.
    /// Returns a unique and stable identifier for the clause.
    pub fn add_clause(&mut self, clause: impl Into<Disjunction>) -> ClauseId {
        self.add_clause_impl(clause.into(), false)
    }

    /// Adds a clause that is implied by the other clauses and that the solver is allowed to forget if
    /// it judges that its constraint database is bloated and that this clause is not helpful in resolution.
    pub fn add_forgettable_clause(&mut self, clause: impl Into<Disjunction>) {
        self.add_clause_impl(clause.into(), true);
    }

    fn add_clause_impl(&mut self, clause: Disjunction, learnt: bool) -> ClauseId {
        let cl_id = self.clauses.add_clause(Clause::new(clause), learnt);
        self.pending_clauses.push_back(cl_id);
        cl_id
    }

    /// Process a newly added clause, making no assumption on the status of the clause.
    ///
    /// The only requirement is that the clause should not have been processed yet.
    fn process_arbitrary_clause(&mut self, cl_id: ClauseId, model: &mut Domains) -> Option<ClauseId> {
        let clause = &self.clauses[cl_id];
        if clause.is_empty() {
            // empty clause is always conflicting
            return Some(cl_id);
        } else if clause.has_single_literal() {
            let l = clause.watch1;
            self.watches.add_watch(cl_id, !l);
            return match model.value(l) {
                None => {
                    self.set_from_unit_propagation(l, cl_id, model);
                    None
                }
                Some(true) => None,
                Some(false) => Some(cl_id),
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
            // violated
            debug_assert!(model.violated_clause(clause));
            self.set_watch_on_first_literals(cl_id);
            Some(cl_id)
        } else if model.value(l1).is_none() {
            // pending, set watch and leave state unchanged
            debug_assert!(model.value(l0).is_none());
            debug_assert!(model.pending_clause(clause));
            self.set_watch_on_first_literals(cl_id);
            None
        } else {
            // clause is unit
            debug_assert!(model.value(l0).is_none());
            debug_assert!(model.unit_clause(clause));
            self.process_unit_clause(cl_id, model);
            None
        }
    }

    fn move_watches_front(&mut self, cl_id: ClauseId, model: &Domains) {
        self.clauses[cl_id].move_watches_front(
            |l| model.value(l),
            |l| {
                debug_assert_eq!(model.value(l), Some(true));
                model.implying_event(l)
            },
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
            let clause = &self.clauses[cl_id];

            let l = clause.watch1;
            debug_assert!(model.value(l).is_none());
            debug_assert!(model.violated_clause(clause.literals().dropping(1)));

            self.set_watch_on_first_literals(cl_id);
            self.set_from_unit_propagation(l, cl_id, model);
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
        while let Some(cl) = self.pending_clauses.pop_front() {
            if let Some(conflict) = self.process_arbitrary_clause(cl, model) {
                self.stats.conflicts += 1;
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
                let watched_literal = watch.to_lit(new_lit.affected_bound());
                let clause = watch.watcher;
                debug_assert!(self.clauses[clause].literals().any(|l| l == !watched_literal));

                // we propagate unless:
                // - we found a contradicting clause earlier
                // - the event does not makes the watched literal true (meaning it was already true before this event)
                if contradicting_clause.is_none() && ev.makes_true(watched_literal) {
                    if !self.propagate_clause(clause, new_lit, model) {
                        // propagation found a contradiction
                        self.stats.conflicts += 1;
                        contradicting_clause = Some(clause);
                    }
                } else {
                    // we encountered a contradicting clause or the event is not triggering,
                    // we need to restore the watch
                    let to_restore = watch.to_lit(new_lit.affected_bound());
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
            // only one literal that is false, the clause is in conflict
            self.watches.add_watch(clause_id, p);
            return false;
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
        // look for replacement for lits[1] : a literal that is not false.
        // we look for them in the unwatched literals.
        for i in 0..clause.unwatched.len() {
            let lit = clause.unwatched[i];
            if !model.entails(!lit) {
                clause.set_watch2(i);
                self.watches.add_watch(clause_id, !lit);
                return true;
            }
        }
        // no replacement found, clause is unit, restore watch and propagate
        self.watches.add_watch(clause_id, !clause.watch2);
        let first_lit = clause.watch1;
        match model.value(first_lit) {
            // TODO: check
            Some(true) => true,
            Some(false) => false,
            None => {
                self.set_from_unit_propagation(first_lit, clause_id, model);
                true
            }
        }
    }

    fn set_from_unit_propagation(&mut self, literal: Lit, propagating_clause: ClauseId, model: &mut Domains) {
        // lock clause to ensure it will not be removed. This is necessary as we might need it to provide an explanation
        self.lock(propagating_clause);
        model.set_unchecked(literal, self.token.cause(propagating_clause));
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
        true
    }

    pub fn explain(&mut self, literal: Lit, cause: u32, model: &Domains, explanation: &mut Explanation) {
        debug_assert_eq!(model.value(literal), None);
        let clause = ClauseId::from(cause);
        // bump the activity of any clause use in an explanation
        self.clauses.bump_activity(clause);
        let clause = &self.clauses[clause];
        debug_assert!(model.unit_clause(clause));
        explanation.reserve(clause.len() - 1);
        for l in clause {
            if l.entails(literal) {
                debug_assert_eq!(model.value(l), None)
            } else {
                debug_assert_eq!(model.value(l), Some(false));
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
        if self.clauses.num_learnt() as i64 - self.locks.num_locks() as i64 >= self.state.allowed_learnt as i64 {
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
}

impl BindSplit for SatSolver {
    fn enforce_true(&mut self, e: &Expr, model: &mut Model) -> BindingResult {
        match e.fun {
            Fun::Or => {
                let mut lits = Vec::with_capacity(e.args.len());
                for &a in &e.args {
                    let a = BAtom::try_from(a).expect("not a boolean");
                    let lit = model.reify(a);
                    lits.push(lit);
                }
                if let Some(clause) = Disjunction::new_non_tautological(lits) {
                    self.add_clause(clause);
                }
                BindingResult::Refined
            }
            _ => BindingResult::Unsupported,
        }
    }

    fn enforce_false(&mut self, expr: &Expr, model: &mut Model) -> BindingResult {
        match expr.fun {
            Fun::Or => {
                // (not (or a b ...))
                //enforce the equivalent (and (not a) (not b) ....)
                for &a in &expr.args {
                    let a = BAtom::try_from(a).expect("not a boolean");
                    let lit = model.reify(a);
                    self.add_clause([!lit]);
                }
                BindingResult::Refined
            }
            _ => BindingResult::Unsupported,
        }
    }

    fn enforce_eq(&mut self, reif: Lit, e: &Expr, model: &mut Model) -> BindingResult {
        match e.fun {
            Fun::Or => {
                // l  <=>  (or a b ...)
                // first, transform all Atoms into literals
                let mut disjuncts = Vec::with_capacity(e.args.len());
                for &a in &e.args {
                    let a = BAtom::try_from(a).expect("not a boolean");
                    let lit = model.reify(a);
                    disjuncts.push(lit);
                }
                let mut clause = Vec::with_capacity(disjuncts.len() + 1);
                // make l => (or a b ...)    <=>   (or (not l) a b ...)
                clause.push(!reif);
                disjuncts.iter().for_each(|l| clause.push(*l));
                if let Some(clause) = Disjunction::new_non_tautological(clause) {
                    self.add_clause(clause);
                }
                // make (or a b ...) => l    <=> (and (a => l) (b => l) ...)
                for disjunct in disjuncts {
                    // enforce a => l
                    let clause = vec![!disjunct, reif];
                    if let Some(clause) = Disjunction::new_non_tautological(clause) {
                        self.add_clause(clause);
                    }
                }
                BindingResult::Refined
            }
            _ => BindingResult::Unsupported,
        }
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

#[cfg(test)]
mod tests {
    use crate::solver::sat_solver::SatSolver;
    use aries_backtrack::Backtrack;
    use aries_model::bounds::Lit;
    use aries_model::extensions::AssignmentExt;
    use aries_model::lang::IntCst;
    use aries_model::state::Cause;
    use aries_model::{Model, WriterId};

    #[test]
    fn test_propagation_simple() {
        let writer = WriterId::new(1u8);
        let model = &mut Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");

        let mut sat = SatSolver::new(writer);
        let a_or_b = vec![Lit::geq(a, 1), Lit::geq(b, 1)];

        sat.add_clause(a_or_b);
        sat.propagate(&mut model.state).unwrap();
        assert_eq!(model.boolean_value_of(a), None);
        assert_eq!(model.boolean_value_of(b), None);
        model.state.set_ub(a, 0, Cause::Decision).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), None);
        sat.propagate(&mut model.state).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), Some(true));
    }

    #[test]
    fn test_propagation_complex() {
        let writer = WriterId::new(1u8);
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
        check_values(&model, [None, None, None, None]);

        let mut sat = SatSolver::new(writer);
        let clause = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.true_lit()];

        sat.add_clause(clause);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [None, None, None, None]);

        model.save_state();
        model.state.decide(a.false_lit()).unwrap();
        check_values(&model, [Some(false), None, None, None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), None, None, None]);

        model.save_state();
        model.state.decide(b.false_lit()).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        model.save_state();
        model.state.decide(c.true_lit()).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), None]);

        model.save_state();
        model.state.decide(d.false_lit()).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), Some(false)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), Some(false)]);

        model.restore_last();
        check_values(&model, [Some(false), Some(false), Some(true), None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), None]);

        model.restore_last();
        check_values(&model, [Some(false), Some(false), None, None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        model.state.decide(c.false_lit()).unwrap();
        check_values(&model, [Some(false), Some(false), Some(false), None]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), Some(false), Some(true)]);
    }

    #[test]
    fn test_propagation_failure() {
        let writer = WriterId::new(1u8);
        let model = &mut Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");

        let mut sat = SatSolver::new(writer);
        let a_or_b = vec![Lit::geq(a, 1), Lit::geq(b, 1)];

        sat.add_clause(a_or_b);
        sat.propagate(&mut model.state).unwrap();
        assert_eq!(model.boolean_value_of(a), None);
        assert_eq!(model.boolean_value_of(b), None);
        model.state.set_ub(a, 0, Cause::Decision).unwrap();
        model.state.set_ub(b, 0, Cause::Decision).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), Some(false));
        assert!(sat.propagate(&mut model.state).is_err());
    }

    #[test]
    fn test_online_clause_insertion() {
        let writer = WriterId::new(1u8);
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
        check_values(&model, [None, None, None, None]);

        // not(a) and not(b)
        model.state.set_ub(a, 0, Cause::Decision).unwrap();
        model.state.set_ub(b, 0, Cause::Decision).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        let abcd = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.true_lit()];
        sat.add_clause(abcd);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        let nota_notb = vec![a.false_lit(), b.false_lit()];
        sat.add_clause(nota_notb);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        let nota_b = vec![a.false_lit(), b.true_lit()];
        sat.add_clause(nota_b);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        let a_b_notc = vec![a.true_lit(), b.true_lit(), c.false_lit()];
        sat.add_clause(a_b_notc);
        sat.propagate(&mut model.state).unwrap(); // should trigger and in turn trigger the first clause
        check_values(&model, [Some(false), Some(false), Some(false), Some(true)]);

        let violated = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.false_lit()];
        sat.add_clause(violated);
        assert!(sat.propagate(&mut model.state).is_err());
    }

    #[test]
    fn test_int_propagation() {
        let writer = WriterId::new(1u8);
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
        check_values(&model, [(0, 10), (0, 10), (0, 10), (0, 10)]);

        let mut sat = SatSolver::new(writer);
        let clause = vec![Lit::leq(a, 5), Lit::leq(b, 5)];
        sat.add_clause(clause);
        let clause = vec![Lit::geq(c, 5), Lit::geq(d, 5)];
        sat.add_clause(clause);

        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(0, 10), (0, 10), (0, 10), (0, 10)]);

        // lower bound changes

        model.state.set_lb(a, 4, Cause::Decision).unwrap();
        check_values(&model, [(4, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(4, 10), (0, 10), (0, 10), (0, 10)]);

        model.state.set_lb(a, 5, Cause::Decision).unwrap();
        check_values(&model, [(5, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(5, 10), (0, 10), (0, 10), (0, 10)]);

        // trigger first clause
        model.save_state();
        sat.save_state();
        model.state.set_lb(a, 6, Cause::Decision).unwrap();
        check_values(&model, [(6, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(6, 10), (0, 5), (0, 10), (0, 10)]);

        // retrigger first clause with stronger literal
        model.restore_last();
        sat.restore_last();
        check_values(&model, [(5, 10), (0, 10), (0, 10), (0, 10)]);
        model.state.set_lb(a, 8, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 10), (0, 10)]);

        // Upper bound changes

        model.state.set_ub(c, 6, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 6), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 6), (0, 10)]);

        model.state.set_ub(c, 5, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 5), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 5), (0, 10)]);

        // should trigger second clause
        model.save_state();
        sat.save_state();
        model.state.set_ub(c, 4, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 4), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 4), (5, 10)]);

        // retrigger second clause with stronger literal
        model.restore_last();
        sat.restore_last();
        check_values(&model, [(8, 10), (0, 5), (0, 5), (0, 10)]);
        model.state.set_ub(c, 2, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 2), (0, 10)]);
        sat.propagate(&mut model.state).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 2), (5, 10)]);
    }
}
