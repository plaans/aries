#![allow(clippy::needless_range_loop)]

pub mod all;
pub mod clause;
pub mod cnf;
pub mod events;
pub mod heuristic;
pub mod stats;

use crate::clause::{Clause, ClauseDB, ClauseId, ClausesParams};
use crate::heuristic::{Heur, HeurParams};
use crate::stats::Stats;
use aries_collections::id_map::IdMap;
use aries_collections::Range;
use std::collections::{HashSet, VecDeque};

use crate::all::*;
use aries_collections::index_map::*;
use aries_collections::Next;
use std::ops::Index;

use crate::SearchStatus::{Conflict, Consistent, Pending, Solution, Unsolvable};
use itertools::Itertools;
use std::f64::NAN;
use std::fmt::{Debug, Formatter};

#[derive(Debug)]
pub enum SearchResult<'a> {
    Solved(Model<'a>),
    Unsolvable,
    Abandoned(AbandonCause),
}

#[derive(Debug, Eq, PartialEq)]
pub enum AbandonCause {
    MaxConflicts,
    Timeout,
    Interrupted,
}

#[derive(Copy, Clone, Debug)]
pub struct SearchParams {
    var_decay: f64,
    cla_decay: f64,
    init_nof_conflict: usize,
    /// Given a problem with N clauses, the number of learnt clause will initially be
    ///     init_learnt_base + N * int_learnt_ratio
    init_learnt_ratio: f64,
    init_learnt_base: f64,
    use_learning: bool,
}
impl Default for SearchParams {
    fn default() -> Self {
        SearchParams {
            var_decay: 0.95,
            cla_decay: 0.999,
            init_nof_conflict: 100,
            init_learnt_ratio: 1_f64 / 3_f64,
            init_learnt_base: 1000_f64,
            use_learning: true,
        }
    }
}

pub struct Solver {
    num_vars: u32,
    pub(crate) assignments: Assignments,
    clauses: ClauseDB,
    watches: IndexMap<Lit, Vec<ClauseId>>,
    propagation_queue: Vec<Lit>,
    heuristic: Heur,
    pub params: SearchParams,
    pub stats: Stats,
    search_state: SearchState,
    /// Buffer use in propagation to avoid new allocations. It will be cleared at the start
    /// of any invocation to `Solver::propagate`
    propagation_work_buffer: Vec<ClauseId>,
    /// Clauses that have been added to the database but have not been processed yet.
    /// In particular, their watches have not been set up.
    pending_clauses: VecDeque<ClauseId>,
}

impl Debug for Solver {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Solver(status: {:?})", self.search_state.status)
    }
}

struct SearchState {
    allowed_conflicts: f64,
    allowed_learnt: f64,
    conflicts_since_restart: usize,
    status: SearchStatus,
}

impl Default for SearchState {
    fn default() -> Self {
        SearchState {
            allowed_conflicts: NAN,
            allowed_learnt: NAN,
            conflicts_since_restart: 0,
            status: SearchStatus::Consistent,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SearchStatus {
    Unsolvable,
    Pending,
    Conflict,
    Consistent,
    Solution,
}

pub enum PropagationResult {
    Unsolvable,
    Conflict(ClauseId),
    Consistent,
    Solution,
}

impl Solver {
    pub fn new(num_vars: u32, params: SearchParams) -> Self {
        let db = ClauseDB::new(ClausesParams::default());
        let watches = IndexMap::new_with(((num_vars + 1) * 2) as usize, Vec::new);

        let mut solver = Solver {
            num_vars,
            assignments: Assignments::new(num_vars),
            clauses: db,
            watches,
            propagation_queue: Vec::new(),
            heuristic: Heur::init(num_vars, HeurParams::default()),
            params,
            stats: Stats::default(),
            search_state: Default::default(),
            propagation_work_buffer: Default::default(),
            pending_clauses: Default::default(),
        };
        solver.stats.init_time = time::precise_time_s();
        solver.check_invariants();
        solver
    }

    pub fn with_clauses(clauses: Vec<Box<[Lit]>>, params: SearchParams) -> Self {
        let mut biggest_var = 0;
        for cl in &clauses {
            for lit in &**cl {
                biggest_var = biggest_var.max(lit.variable().id.get())
            }
        }
        let db = ClauseDB::new(ClausesParams::default());
        let watches = IndexMap::new_with(((biggest_var + 1) * 2) as usize, Vec::new);

        let mut solver = Solver {
            num_vars: biggest_var,
            assignments: Assignments::new(biggest_var),
            clauses: db,
            watches,
            propagation_queue: Vec::new(),
            heuristic: Heur::init(biggest_var, HeurParams::default()),
            params,
            stats: Default::default(),
            search_state: Default::default(),
            propagation_work_buffer: Default::default(),
            pending_clauses: Default::default(),
        };

        for cl in clauses {
            solver.add_clause(&cl);
        }

        solver.check_invariants();
        solver.stats.init_time = time::precise_time_s();
        solver
    }

    /// Select the two literals to watch and move them to the first 2 literals of the clause.
    ///
    /// After clause[0] will be the element with the highest priority and clause[1] the one with
    /// the second highest priority. Order of other elements is undefined.
    ///
    /// Priority is defined as follows:
    ///   - TRUE literals
    ///   - UNDEF literals
    ///   - FALSE Literal, prioritizing those with the highest decision level
    ///   - left most literal in the original clause (to avoid swapping two literals with the same priority)
    fn move_watches_front(&mut self, cl_id: ClauseId) {
        fn priority(s: &Assignments, lit: Lit) -> DecisionLevel {
            match s.value_of(lit) {
                BVal::Undef => DecisionLevel::MAX.prev(),
                BVal::True => DecisionLevel::MAX,
                BVal::False => s.level(lit.variable()),
            }
        }
        let cl = &mut self.clauses[cl_id].disjuncts;
        debug_assert!(cl.len() >= 2);
        let mut lvl0 = priority(&self.assignments, cl[0]);
        let mut lvl1 = priority(&self.assignments, cl[1]);
        if lvl1 > lvl0 {
            std::mem::swap(&mut lvl0, &mut lvl1);
            cl.swap(0, 1);
        }
        for i in 2..cl.len() {
            let lvl = priority(&self.assignments, cl[i]);
            if lvl > lvl1 {
                lvl1 = lvl;
                cl.swap(1, i);
                if lvl > lvl0 {
                    lvl1 = lvl0;
                    lvl0 = lvl;
                    cl.swap(0, 1);
                }
            }
        }
        let cl = &self.clauses[cl_id].disjuncts;
        debug_assert_eq!(lvl0, priority(&self.assignments, cl[0]));
        debug_assert_eq!(lvl1, priority(&self.assignments, cl[1]));
        debug_assert!(lvl0 >= lvl1);
        debug_assert!(cl[2..].iter().all(|l| lvl1 >= priority(&self.assignments, *l)));
    }

    /// et the watch on the first two literals
    fn set_watch_on_first_literals(&mut self, cl_id: ClauseId) {
        let cl = &self.clauses[cl_id].disjuncts;
        debug_assert!(cl.len() >= 2);
        self.watches[!cl[0]].push(cl_id);
        self.watches[!cl[1]].push(cl_id);
        debug_assert!(self.assert_watches_valid(cl_id));
    }

    fn assert_watches_valid(&self, cl_id: ClauseId) -> bool {
        let cl = &self.clauses[cl_id].disjuncts;
        let l0 = cl[0];
        let l1 = cl[1];
        assert!(self.watches[!l0].contains(&cl_id));
        assert!(self.watches[!l1].contains(&cl_id));
        if self.satisfied(cl) {
            assert!(self.is_set(l0) || self.is_set(l1));
        } else if self.pending(cl) {
            assert!(self.is_undef(l0) && self.is_undef(l1));
        } else if self.violated(cl) {
        }
        true
    }

    pub fn variables(&self) -> Range<BVar> {
        BVar::first(self.num_vars as usize)
    }

    pub fn set_polarity(&mut self, variable: BVar, default_value: bool) {
        self.assignments.ass[variable].polarity = default_value;
    }

    pub fn decide(&mut self, decision: Lit) {
        self.check_invariants();
        self.assignments.add_backtrack_point(decision);
        self.assume(decision, None);
    }

    fn assume(&mut self, decision: Lit, reason: Option<ClauseId>) {
        self.check_invariants();
        self.assignments.set_lit(decision, reason);
        self.propagation_queue.push(decision);
        self.check_invariants();
    }

    /// Returns:
    ///   Some(i): in case of a conflict where i is the id of the violated clause
    ///   None if no conflict was detected during propagation
    fn propagate_enqueued(&mut self) -> Option<ClauseId> {
        debug_assert!(
            self.pending_clauses.is_empty(),
            "Some clauses have not been integrated in the database yet."
        );
        self.check_invariants();
        while !self.propagation_queue.is_empty() {
            let p = self.propagation_queue.pop().unwrap();
            self.propagation_work_buffer.clear();
            for x in self.watches[p].drain(..) {
                self.propagation_work_buffer.push(x);
            }

            let n = self.propagation_work_buffer.len();
            for i in 0..n {
                if !self.propagate_clause(self.propagation_work_buffer[i], p) {
                    // clause violated
                    // restore remaining watches
                    for j in i + 1..n {
                        self.watches[p].push(self.propagation_work_buffer[j]);
                    }
                    self.propagation_queue.clear();
                    self.check_invariants();
                    self.search_state.status = SearchStatus::Conflict;
                    return Some(self.propagation_work_buffer[i]);
                }
            }
        }
        self.check_invariants();
        None
    }

    fn propagate_clause(&mut self, clause_id: ClauseId, p: Lit) -> bool {
        self.stats.propagations += 1;
        let lits = &mut self.clauses[clause_id].disjuncts;
        if lits[0] == !p {
            lits.swap(0, 1);
        }
        debug_assert!(lits[1] == !p);
        debug_assert!(self.value_of(!p) == BVal::False);
        let lits = &self.clauses[clause_id].disjuncts;
        if self.is_set(lits[0]) {
            // clause satisfied, restore the watch and exit
            self.watches[p].push(clause_id);
            //            self.check_invariants();
            return true;
        }
        for i in 2..lits.len() {
            if !self.is_set(!lits[i]) {
                let lits = &mut self.clauses[clause_id].disjuncts;
                lits.swap(1, i);
                self.watches[!lits[1]].push(clause_id);
                //                self.check_invariants();
                return true;
            }
        }
        // no replacement found, clause is unit
        self.watches[p].push(clause_id);
        let first_lit = lits[0];
        self.enqueue(first_lit, Some(clause_id))
    }
    fn value_of(&self, lit: Lit) -> BVal {
        self.assignments.value_of(lit)
    }

    /// Returns the value of the given variable or None if it not set.
    pub fn get_variable(&self, variable: BVar) -> Option<bool> {
        match self.assignments.value_of(variable.true_lit()) {
            BVal::Undef => None,
            BVal::True => Some(true),
            BVal::False => Some(false),
        }
    }

    /// Returns the value of the given literal or None if it not set.
    pub fn get_literal(&self, literal: Lit) -> Option<bool> {
        match self.assignments.value_of(literal) {
            BVal::Undef => None,
            BVal::True => Some(true),
            BVal::False => Some(false),
        }
    }

    fn is_undef(&self, lit: Lit) -> bool {
        self.assignments.get(lit.variable()) == BVal::Undef
    }
    fn is_set(&self, lit: Lit) -> bool {
        match self.assignments.get(lit.variable()) {
            BVal::Undef => false,
            BVal::True => lit.is_positive(),
            BVal::False => lit.is_negative(),
        }
    }

    /// Returns false if the given literal is already negated.
    /// Otherwise, adds the literal to the propagation queue and returns true.
    fn enqueue(&mut self, lit: Lit, reason: Option<ClauseId>) -> bool {
        if let Some(r) = reason {
            // check that the clause implies the literal
            debug_assert!(self.clauses[r].disjuncts.iter().all(|&l| self.is_set(!l) || l == lit));
        }
        if self.is_set(!lit) {
            // contradiction
            false
        } else if self.is_set(lit) {
            // already known
            true
        } else {
            // enqueue lit
            self.assignments.set(lit.variable(), lit.is_positive(), reason);
            self.propagation_queue.push(lit);
            //            self.check_invariants();
            self.search_state.status = SearchStatus::Pending;
            true
        }
    }

    fn analyze(&mut self, original_clause: ClauseId) -> (Vec<Lit>, DecisionLevel) {
        // TODO: many allocations to optimize here
        let mut seen = vec![false; self.num_vars as usize + 1]; // todo: use a bitvector
        let mut counter = 0;
        let mut p = None;
        let mut p_reason = Vec::new();
        let mut out_learnt = Vec::new();
        let mut out_btlevel = GROUND_LEVEL;

        {
            // some sanity check
            let analyzed = &self.clauses[original_clause].disjuncts;
            // all variables should be false
            debug_assert!(self.violated(&analyzed));
            // at least one variable should have been set at the current level
            debug_assert!(analyzed
                .iter()
                .any(|&lit| self.assignments.level(lit.variable()) == self.assignments.decision_level()));
        }
        let mut clause = Some(original_clause);
        let mut simulated_undone = 0;

        // first literal will be the one on which we backtrack
        out_learnt.push(Lit::dummy());

        let mut first = true;
        while first || counter > 0 {
            first = false;
            p_reason.clear();
            debug_assert!(clause.is_some(), "Analyzed clause is empty.");
            // extract to p_reason the conjunction of literal that made p true (negation of all
            // other literals in the clause).
            // negation of all literals in the clause if p is none
            self.calc_reason(clause.unwrap(), p, &mut p_reason);

            for &q in &p_reason {
                let qvar = q.variable();
                if !seen[q.variable().to_index()] {
                    seen[q.variable().to_index()] = true;
                    if self.assignments.level(qvar) == self.assignments.decision_level() {
                        counter += 1;
                        // check that that the variable is not in the undone part of the trail
                        debug_assert!(
                            (0..simulated_undone).all(|i| self.assignments.last_assignment(i).variable() != qvar)
                        );
                    } else if self.assignments.level(qvar) > GROUND_LEVEL {
                        out_learnt.push(!q);
                        out_btlevel = out_btlevel.max(self.assignments.level(qvar));
                    }
                }
            }

            // go to next seen variable
            while {
                // do
                let l = self.assignments.last_assignment(simulated_undone);
                debug_assert!(self.assignments.is_set(l.variable()));
                debug_assert!(self.assignments.level(l.variable()) == self.assignments.decision_level());
                p = Some(l);
                clause = self.assignments.reason(l.variable());

                simulated_undone += 1;

                // while
                !seen[l.variable().to_index()]
            } {}
            debug_assert!(seen[p.unwrap().variable().to_index()]);
            counter -= 1;
        }
        debug_assert!(out_learnt[0] == Lit::dummy());
        out_learnt[0] = !p.unwrap();

        (out_learnt, out_btlevel)
    }

    fn calc_reason(&mut self, clause: ClauseId, op: Option<Lit>, out_reason: &mut Vec<Lit>) {
        let cl = &self.clauses[clause];
        debug_assert!(out_reason.is_empty());
        debug_assert!(op.iter().all(|&p| cl.disjuncts[0] == p));
        let first = match op {
            Some(_) => 1,
            None => 0,
        };
        for &l in &cl.disjuncts[first..] {
            out_reason.push(!l);
        }
        if cl.learnt {
            self.clauses.bump_activity(clause)
        }
    }

    /// Backtrack one level, returning the decision that was undone
    fn backtrack(&mut self) -> Option<Lit> {
        let h = &mut self.heuristic;
        self.assignments.backtrack(&mut |v| h.var_insert(v))
    }

    /// Backtracks until decision level `lvl` and returns the decision literal associated with this level
    /// (which is the last undone decision).
    /// Returns None if nothing was undone (i.e; the current decision level was already `>= lvl`)
    /// Outcome the decision level of the solver is lesser than or equal to the one requested.
    fn backtrack_to(&mut self, lvl: DecisionLevel) -> Option<Lit> {
        let h = &mut self.heuristic;
        self.assignments.backtrack_to(lvl, &mut |v| h.var_insert(v))
    }
    fn satisfied(&self, clause: &[Lit]) -> bool {
        clause.iter().any(|&lit| self.is_set(lit))
    }
    fn violated(&self, clause: &[Lit]) -> bool {
        clause.iter().all(|&lit| self.is_set(!lit))
    }
    fn unit(&self, clause: &[Lit]) -> bool {
        !self.satisfied(clause) && clause.iter().filter(|lit| self.is_undef(**lit)).take(2).count() == 1
    }
    fn pending(&self, clause: &[Lit]) -> bool {
        !self.satisfied(clause) && clause.iter().filter(|lit| self.is_undef(**lit)).take(2).count() == 2
    }

    /// This is the public version of `handle_conflict_impl` that will check the typical assumptions
    /// and panic if they are not satisfied.
    pub fn handle_conflict(&mut self, conflicting: ClauseId) {
        assert_eq!(
            self.search_state.status,
            SearchStatus::Conflict,
            "Solver is not in conflicting state"
        );
        let cl = &self.clauses[conflicting].disjuncts;
        assert!(self.violated(cl), "Clause is not violated.");
        let lvl = self.assignments.level(cl[0].variable());
        // TODO: can we arbritrarily select the bactrack level as the one of the first literal ?
        assert!(lvl <= self.assignments.decision_level(), "");
        self.backtrack_to(lvl);
        self.handle_conflict_impl(conflicting, self.params.use_learning);
    }

    fn handle_conflict_impl(&mut self, conflict: ClauseId, use_learning: bool) {
        // debug_assert_eq!(self.search_state.status, SearchStatus::Conflict); // TODO: uncomment
        debug_assert!(self.violated(&self.clauses[conflict].disjuncts));
        debug_assert_eq!(
            self.assignments.level(self.clauses[conflict].disjuncts[0].variable()),
            self.assignments.decision_level()
        );

        self.stats.conflicts += 1;
        self.search_state.conflicts_since_restart += 1;

        if self.assignments.decision_level() == self.assignments.root_level() {
            self.search_state.status = SearchStatus::Unsolvable;
        } else if use_learning {
            let (learnt_clause, backtrack_level) = self.analyze(conflict);
            debug_assert!(backtrack_level < self.assignments.decision_level());
            debug_assert!(self.violated(&learnt_clause));
            match self.backtrack_to(backtrack_level) {
                Some(_dec) => (), // backtracked, decision !_dec will be enforced by the learned clause
                None => {
                    // no decision left to undo
                    self.search_state.status = Unsolvable;
                    return;
                }
            }
            debug_assert!(self.unit(&learnt_clause));
            let learnt_id = self.clauses.add_clause(Clause::new(&learnt_clause, true));
            self.bump_activity_on_learnt_from_conflict(learnt_id);
            self.process_unit_clause(learnt_id);
        } else {
            // no learning
            match self.backtrack() {
                Some(dec) => {
                    // backtracking: !dec
                    self.assume(!dec, None);
                    self.search_state.status = SearchStatus::Consistent;
                }
                None => {
                    self.search_state.status = SearchStatus::Unsolvable; // no decision left to undo
                }
            }
        }
        if self.search_state.status == Solution {
            debug_assert!(self.is_model_valid());
        }
    }

    /// Bump activity of the clause and of all its variables
    fn bump_activity_on_learnt_from_conflict(&mut self, cl_id: ClauseId) {
        for l in &self.clauses[cl_id].disjuncts {
            self.heuristic.var_bump_activity(l.variable());
        }
        self.clauses.bump_activity(cl_id);
    }

    fn process_unit_clause(&mut self, cl_id: ClauseId) -> SearchStatus {
        let clause = &self.clauses[cl_id].disjuncts;
        debug_assert!(self.unit(clause));

        if clause.len() == 1 {
            let l = clause[0];
            debug_assert!(self.is_undef(l));
            // TODO: we can probably resort to enqueue like in the other case
            debug_assert!(!self.watches[!l].contains(&cl_id));
            // watch the only literal
            self.watches[!l].push(cl_id);
            self.enforce_singleton_clause(l);
        } else {
            debug_assert!(clause.len() >= 2);

            // Set up watch, the first literal must be undefined and the others violated.
            self.move_watches_front(cl_id);
            let l = self.clauses[cl_id].disjuncts[0];
            debug_assert!(self.is_undef(l));
            debug_assert!(self.violated(&self.clauses[cl_id].disjuncts[1..]));
            self.set_watch_on_first_literals(cl_id);
            self.enqueue(l, Some(cl_id));
        }

        self.search_state.status
    }

    /// Handles a clause with a single literal. Depending on parameters, it can either
    /// propagate the literal and forget about it or backtrack all the way back to root
    /// level before enqueueing the literal. In the latter case, the clause will not be lost.
    ///
    /// This is the way things work in minisat but we might be able to get the best of both worlds
    /// with restart + phase_saving (not touching the conflict limits by not marking as restarted?)
    /// or with special support in the trail.
    /// TODO: doc is outdated and this function backtracks and does not set the reason for the literal
    fn enforce_singleton_clause(&mut self, lit: Lit) {
        self.backtrack_to(self.assignments.root_level());
        self.search_state.status = SearchStatus::Consistent;

        debug_assert_eq!(self.assignments.decision_level(), self.assignments.root_level());
        if !self.enqueue(lit, None) {
            // literal is already false at root level, there is no solution to this problem.
            self.search_state.status = Unsolvable;
        }
    }

    /// Return None if no solution was found within the conflict limit.
    fn search(&mut self) -> SearchResult {
        loop {
            match self.propagate() {
                Some(conflict) => {
                    self.handle_conflict(conflict); // TODO: use handle_conflict_impl ?
                                                    // self.handle_conflict_impl(conflict, self.params.use_learning);
                    match self.search_state.status {
                        SearchStatus::Unsolvable => {
                            self.stats.end_time = time::precise_time_s();
                            return SearchResult::Unsolvable;
                        }
                        SearchStatus::Consistent | SearchStatus::Pending => (),
                        x => unreachable!("{:?}", x),
                    }
                    self.decay_activities()
                }
                None => {
                    if self.assignments.decision_level() == GROUND_LEVEL {
                        // TODO: simplify db
                    }
                    if self.clauses.num_learnt() as i64 - self.assignments.num_assigned() as i64
                        >= self.search_state.allowed_learnt as i64
                    {
                        // todo use a bitset
                        let locked = self
                            .variables()
                            .filter_map(|var| self.assignments.reason(var))
                            .collect::<HashSet<_>>();
                        let watches = &mut self.watches;
                        self.clauses.reduce_db(|cl| locked.contains(&cl), watches);
                    }

                    if self.num_vars() as usize == self.assignments.num_assigned() {
                        // model found
                        debug_assert!(self.is_model_valid());
                        self.stats.end_time = time::precise_time_s();
                        self.search_state.status = SearchStatus::Solution;
                        return SearchResult::Solved(Model(self));
                    } else if self.search_state.conflicts_since_restart > self.search_state.allowed_conflicts as usize {
                        // reached bound on number of conflicts
                        // cancel until root level
                        self.backtrack_to(self.assignments.root_level());
                        self.stats.end_time = time::precise_time_s();
                        self.search_state.status = Pending;
                        return SearchResult::Abandoned(AbandonCause::MaxConflicts);
                    } else {
                        let next: BVar = loop {
                            match self.heuristic.next_var() {
                                Some(v) if !self.assignments.is_set(v) => break v, // // not set, select for decision
                                Some(_) => continue,                               // var already set, proceed to next
                                None => panic!("No unbound value in the heap."),
                            }
                        };

                        let polarity = self.assignments.ass[next].polarity;
                        self.decide(next.lit(polarity));
                        self.stats.decisions += 1;
                    }
                }
            }
        }
    }
    fn num_vars(&self) -> u32 {
        self.num_vars
    }

    fn decay_activities(&mut self) {
        self.clauses.decay_activities();
        self.heuristic.decay_activities();
    }

    pub fn solve(&mut self) -> SearchResult {
        match self.search_state.status {
            SearchStatus::Unsolvable => return SearchResult::Unsolvable,
            SearchStatus::Solution => {
                debug_assert!(self.is_model_valid());
                debug_assert!(self.variables().all(|v| !self.is_undef(v.true_lit())));
                // already at a solution, exit immediately
                return SearchResult::Solved(Model(self));
            }
            SearchStatus::Consistent | SearchStatus::Conflict | SearchStatus::Pending => {
                // will keep going,
                // check if we have any unset parameter and set the to default
                if self.search_state.allowed_conflicts.is_nan() {
                    self.search_state.allowed_conflicts = self.params.init_nof_conflict as f64;
                }
                if self.search_state.allowed_learnt.is_nan() {
                    self.search_state.allowed_learnt = self.params.init_learnt_base
                        + self.clauses.num_clauses() as f64 * self.params.init_learnt_ratio;
                }
            }
        }

        loop {
            match self.search() {
                SearchResult::Solved(model) => {
                    debug_assert!(std::ptr::eq(model.0, self));
                    debug_assert!(self.is_model_valid());
                    debug_assert!(!self.variables().any(|v| self.is_undef(v.true_lit())));
                    return SearchResult::Solved(Model(self));
                }
                SearchResult::Abandoned(AbandonCause::MaxConflicts) => {
                    // no decision made within conflict bounds
                    // increase bounds before next run
                    debug_assert_eq!(
                        self.assignments.decision_level(),
                        self.assignments.root_level(),
                        "Search did not backtrack on abandon"
                    );
                    self.search_state.allowed_conflicts *= 1.5;
                    self.search_state.allowed_learnt *= 1.1;
                    self.stats.restarts += 1;
                }
                SearchResult::Unsolvable => return SearchResult::Unsolvable,
                _ => unreachable!(),
            }
        }
    }

    /// Adds a new clause that will be part of the problem definition.
    /// Returns a unique and stable identifier for the clause.
    ///
    /// When
    pub fn add_clause(&mut self, clause: &[Lit]) -> ClauseId {
        self.add_clause_impl(clause, false)
    }

    /// Adds a clause that is implied by the other clauses and that the solver is allowed to forget if
    /// it judges that its constraint database is bloated and that this clause is not helpful in resolution.
    pub fn add_forgettable_clause(&mut self, clause: &[Lit]) {
        self.add_clause_impl(clause, true);
    }

    fn add_clause_impl(&mut self, clause: &[Lit], learnt: bool) -> ClauseId {
        debug_assert!(
            vec![Consistent, Pending, Solution].contains(&self.search_state.status),
            "status: {:?}",
            self.search_state.status
        );
        let cl_id = self.clauses.add_clause(Clause::new(&clause, learnt));
        self.pending_clauses.push_back(cl_id);
        self.search_state.status = Pending;
        cl_id
    }

    pub fn propagate(&mut self) -> Option<ClauseId> {
        debug_assert!(
            vec![Consistent, Pending].contains(&self.search_state.status),
            "Wrong status: {:?}",
            self.search_state.status
        );
        // process all clauses that have been added since last propagation
        while let Some(cl) = self.pending_clauses.pop_front() {
            if let Some(conflict) = self.process_arbitrary_clause(cl) {
                debug_assert_eq!(self.search_state.status, Conflict);
                return Some(conflict);
            }
        }

        self.propagate_enqueued()
    }

    /// Process a newly added clause, making no assumption on the status of the clause.
    ///
    /// The only requirement is that the clause should not have been processed yet.
    fn process_arbitrary_clause(&mut self, cl_id: ClauseId) -> Option<ClauseId> {
        let clause = &self.clauses[cl_id].disjuncts;
        if clause.is_empty() {
            // empty clause, nothing to do
            return None;
        } else if clause.len() == 1 {
            let l = clause[0];
            self.watches[!l].push(cl_id);
            match self.value_of(l) {
                BVal::Undef => {
                    // TODO: process unit clause ?
                    self.enqueue(l, Some(cl_id));
                    return None;
                }
                BVal::True => return None,
                BVal::False => {
                    self.search_state.status = Conflict;
                    return Some(cl_id);
                }
            }
        }

        // clause has at least two literals
        self.move_watches_front(cl_id);
        let clause = &self.clauses[cl_id].disjuncts;
        let l0 = clause[0];
        let l1 = clause[1];

        if self.is_set(l0) {
            // satisfied, set watchers and leave state unchanged
            self.set_watch_on_first_literals(cl_id);
            None
        } else if self.is_set(!l0) {
            // violated
            debug_assert!(self.violated(&clause));
            self.set_watch_on_first_literals(cl_id);
            self.search_state.status = Conflict;
            Some(cl_id)
        } else if self.is_undef(l1) {
            // pending, set watch and leave state unchanged
            debug_assert!(self.is_undef(l0));
            debug_assert!(self.pending(&clause));
            self.set_watch_on_first_literals(cl_id);
            None
        } else {
            // clause is unit
            debug_assert!(self.is_undef(l0));
            debug_assert!(self.unit(&clause));
            self.process_unit_clause(cl_id);
            None
        }
    }

    pub fn model(&self) -> IdMap<BVar, BVal> {
        let mut m = IdMap::new();
        for var in self.variables() {
            let val = self.assignments.ass.get(var).value;
            m.insert(var, val);
        }
        m
    }

    fn is_model_valid(&self) -> bool {
        self.check_invariants();
        for cl_id in self.clauses.all_clauses() {
            let mut is_sat = false;
            for lit in &self.clauses[cl_id].disjuncts {
                if self.is_set(*lit) {
                    is_sat = true;
                }
            }
            if !is_sat {
                println!(
                    "Invalid clause: {}: {} = {:?}",
                    cl_id,
                    self.clauses[cl_id],
                    self.clauses[cl_id]
                        .disjuncts
                        .iter()
                        .map(|l| self.value_of(*l))
                        .collect_vec()
                );
                return false;
            }
        }
        true
    }

    #[cfg(not(feature = "full_check"))]
    fn check_invariants(&self) {}

    #[cfg(feature = "full_check")]
    fn check_invariants(&self) {
        let mut watch_count = IndexMap::new(self.clauses.num_clauses() * 3, 0);
        for watches_for_lit in &self.watches.values[1..] {
            for watcher in watches_for_lit {
                watch_count[*watcher] += 1;
            }
        }
        assert!(self.clauses.all_clauses().all(|n| watch_count[n] == 2));
    }
}

/// Valid total assignment to variables of a given problem.
/// This simply wraps a solver in the `SearchStatus::Solution` state and provide direct
/// access to the value of the variables (without wrapping in `Option`)
#[derive(Debug)]
pub struct Model<'a>(&'a Solver);

impl<'a> Model<'a> {
    fn get_value(&self, var: BVar) -> bool {
        debug_assert_eq!(self.0.search_state.status, SearchStatus::Solution);
        self.0.get_variable(var).expect("Unassigned variable in a model")
    }
    fn get_literal_value(&self, lit: Lit) -> bool {
        let var_val = self.get_value(lit.variable());
        if lit.is_positive() {
            var_val
        } else {
            !var_val
        }
    }
    pub fn assignments(&self) -> impl Iterator<Item = (BVar, bool)> + '_ {
        self.0.variables().map(move |v| (v, self.get_value(v)))
    }
    pub fn literals(&self) -> impl Iterator<Item = Lit> + '_ {
        self.assignments()
            .map(move |(var, val)| if val { var.true_lit() } else { var.false_lit() })
    }
}

impl<'a> Index<Lit> for Model<'a> {
    type Output = bool;
    fn index(&self, index: Lit) -> &Self::Output {
        if self.get_literal_value(index) {
            &true
        } else {
            &false
        }
    }
}
impl<'a> Index<BVar> for Model<'a> {
    type Output = bool;
    fn index(&self, index: BVar) -> &Self::Output {
        if self.get_value(index) {
            &true
        } else {
            &false
        }
    }
}

impl<'a> Index<i32> for Model<'a> {
    type Output = bool;
    fn index(&self, index: i32) -> &Self::Output {
        if self.get_literal_value(Lit::from(index)) {
            &true
        } else {
            &false
        }
    }
}

impl Index<i32> for Solver {
    type Output = Option<bool>;
    fn index(&self, index: i32) -> &Self::Output {
        match self.get_literal(Lit::from(index)) {
            Some(true) => &Some(true),
            Some(false) => &Some(false),
            None => &None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cnf::CNF;
    use crate::SearchResult::Solved;
    use matches::debug_assert_matches;
    use std::fs;

    #[test]
    fn test_add() {
        let a = BVar::from_bits(1);
        let at = a.true_lit();
        assert_eq!(at.id.get(), 1 * 2 + 1);
        let af = a.false_lit();
        assert_eq!(af.id.get(), 1 * 2);
        assert_eq!(a, at.variable());
        assert_eq!(a, af.variable());
        assert_ne!(at, af);
    }
    #[test]
    #[should_panic]
    fn test_invalid_zero() {
        BVar::from_bits(0);
    }
    #[test]
    #[should_panic]
    fn test_invalid_too_big() {
        BVar::from_bits(std::u32::MAX);
    }

    macro_rules! clause {
        ( $( $x:expr ),* ) => {
            {
                let mut temp_vec = Vec::with_capacity(8);
                $(
                    temp_vec.push(Lit::from($x));
                )*
                temp_vec
            }
        };
    }

    #[test]
    fn test_construction() {
        let mut solver = Solver::new(4, SearchParams::default());
        println!("{:?}", clause!(-1, 2));
        solver.add_clause(&clause!(-1, 2));
        assert_eq!(solver[-1], None);
        assert_eq!(solver[2], None);
        assert!(solver.propagate().is_none());
        solver.add_clause(&clause!(-1));
        assert_eq!(solver[-1], None);
        assert_eq!(solver[2], None);
        assert!(solver.propagate().is_none());
        assert_eq!(solver[-1], Some(true));
        assert_eq!(solver[2], None);
        solver.add_clause(&clause!(1));

        assert!(solver.propagate().is_some());
    }

    #[test]
    fn test_full_solver() {
        fn solve(file: &str, expected_result: bool) {
            let file_content = fs::read_to_string(file).unwrap_or_else(|_| panic!("Cannot read file: {}", file));

            let clauses = CNF::parse(&file_content).expect("Invalid file content").clauses;

            let mut solver = Solver::with_clauses(clauses, SearchParams::default());
            let res = solver.solve();
            if expected_result {
                debug_assert_matches!(res, Solved(_)); // Expected solvable but no solution found
            } else {
                debug_assert_matches!(res, SearchResult::Unsolvable); // Expected UNSAT
            }
        }

        solve("../problems/cnf/sat/1.cnf", true);
        solve("../problems/cnf/sat/2.cnf", true);
        solve("../problems/cnf/unsat/1.cnf", false);
        solve("../problems/cnf/unsat/1.cnf", false);
    }
}
