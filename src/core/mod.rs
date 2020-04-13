pub mod all;
pub mod clause;
pub mod cnf;
pub mod events;
pub mod heuristic;
pub mod stats;

use crate::collection::Range;
use crate::collection::id_map::IdMap;
use crate::core::clause::{Clause, ClauseDB, ClauseId, ClausesParams};
use crate::core::heuristic::{Heur, HeurParams};
use crate::core::stats::Stats;
use std::collections::HashSet;

use crate::collection::index_map::*;
use crate::collection::Next;
use crate::core::all::*;
use std::ops::Not;

use log::{info, trace};
use std::f64::NAN;
use crate::core::SearchStatus::{Unsolvable, Restarted, Solution};

#[derive(Debug, Clone, Copy)]
pub enum Decision {
    True(BVar),
    False(BVar),
}
impl Not for Decision {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Decision::True(v) => Decision::False(v),
            Decision::False(v) => Decision::True(v),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SearchParams {
    var_decay: f64,
    cla_decay: f64,
    init_nof_conflict: usize,
    init_learnt_ratio: f64,
    use_learning: bool,
}
impl Default for SearchParams {
    fn default() -> Self {
        SearchParams {
            var_decay: 0.95,
            cla_decay: 0.999,
            init_nof_conflict: 100,
            init_learnt_ratio: 1_f64 / 3_f64,
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
    search_state: SearchState
}

struct SearchState {
    allowed_conflicts: f64,
    allowed_learnt: f64,
    conflicts_since_restart: usize,
    status: SearchStatus
}

impl Default for SearchState {
    fn default() -> Self {
        SearchState {
            allowed_conflicts: NAN,
            allowed_learnt: NAN,
            conflicts_since_restart: 0,
            status: SearchStatus::Init
        }
    }
}


enum AddClauseRes {
    Inconsistent,
    Unit(Lit),
    Complete(ClauseId),
}

// TODO : generalize usage
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SearchStatus {
    Init,
    Unsolvable,
    Ongoing,
    Restarted,
    Solution
}

impl Solver {

    pub fn new(num_vars: u32, params: SearchParams) -> Self {
        let db = ClauseDB::new(ClausesParams::default());
        let watches = IndexMap::new_with(((num_vars+1) * 2) as usize, || Vec::new());

        let solver = Solver {
            num_vars: num_vars,
            assignments: Assignments::new(num_vars),
            clauses: db,
            watches,
            propagation_queue: Vec::new(),
            heuristic: Heur::init(num_vars, HeurParams::default()),
            params: params,
            stats: Stats::default(),
            search_state: Default::default()
        };
        solver.check_invariants();
        solver
    }

    pub fn init(clauses: Vec<Box<[Lit]>>, params: SearchParams) -> Self {
        let mut biggest_var = 0;
        for cl in &clauses {
            for lit in &**cl {
                biggest_var = biggest_var.max(lit.variable().id.get())
            }
        }
        let db = ClauseDB::new(ClausesParams::default());
        // TODO: do we need this +1
        let watches = IndexMap::new_with(((biggest_var + 1) * 2) as usize, || Vec::new());

        let mut solver = Solver {
            num_vars: biggest_var,
            assignments: Assignments::new(biggest_var),
            clauses: db,
            watches,
            propagation_queue: Vec::new(),
            heuristic: Heur::init(biggest_var, HeurParams::default()),
            params,
            stats: Default::default(),
            search_state: Default::default()
        };

        for cl in clauses {
            solver.add_clause(&*cl, false);
        }

        solver.check_invariants();
        solver
    }

    fn add_clause(&mut self, lits: &[Lit], learnt: bool) -> AddClauseRes {
        // TODO: normalize non learnt clauses
        // TODO: support addition of non-learnt clauses during search
        //       This mainly requires making sure the first two literals will be the first two to be unset on backtrack
        //       It also requires handling the case where the clause is unit/violated (in caller)

        if learnt {
            // invariant: at this point we should have undone the assignment to the first literal
            // and all others should still be violated
            // TODO : add check that first literal is unset
            debug_assert!(lits[1..]
                .iter()
                .all(|l| self.assignments.is_set(l.variable())));
        }

        match lits.len() {
            0 => AddClauseRes::Inconsistent,
            1 => {
                //                self.enqueue(lits[0], None);
                AddClauseRes::Unit(lits[0])
            }
            _ => {
                let mut cl = Clause::new(lits, learnt);
                if learnt {
                    // lits[0] is the first literal to watch
                    // select second literal to watch (the one with highest decision level)
                    // and move it to lits[1]
                    let lits = &mut cl.disjuncts;
                    let mut max_i = 1;
                    let mut max_lvl = self.assignments.level(lits[1].variable());
                    for i in 1..lits.len() {
                        let lvl_i = self.assignments.level(lits[i].variable());
                        if lvl_i > max_lvl {
                            max_i = i;
                            max_lvl = lvl_i;
                        }
                    }
                    lits.swap(1, max_i);

                    // adding a learnt clause, we must bump the activity of all its variables
                    for l in lits {
                        self.heuristic.var_bump_activity(l.variable());
                    }
                }
                // the two literals to watch
                let lit0 = cl.disjuncts[0];
                let lit1 = cl.disjuncts[1];
                let cl_id = self.clauses.add_clause(cl);

                self.watches[!lit0].push(cl_id);
                self.watches[!lit1].push(cl_id);
                AddClauseRes::Complete(cl_id)
            }
        }
    }

    pub fn variables(&self) -> Range<BVar> {
        BVar::first(self.num_vars as usize)
    }

    pub fn decide(&mut self, dec: Decision) {
        self.check_invariants();
        trace!("decision: {:?}", dec);
        self.assignments.add_backtrack_point(dec);
        self.assume(dec, None);
    }
    pub fn assume(&mut self, dec: Decision, reason: Option<ClauseId>) {
        self.check_invariants();
        match dec {
            Decision::True(var) => {
                self.assignments.set(var, true, reason);
                self.propagation_queue.push(var.lit(true));
            }
            Decision::False(var) => {
                self.assignments.set(var, false, reason);
                self.propagation_queue.push(var.lit(false));
            }
        }
        self.check_invariants();
    }

    /// Returns:
    ///   Some(i): in case of a conflict where i is the id of the violated clause
    ///   None if no conflict was detected during propagation
    pub fn propagate(&mut self, working: &mut Vec<ClauseId>) -> Option<ClauseId> {
        self.check_invariants();
        while !self.propagation_queue.is_empty() {
            let p = self.propagation_queue.pop().unwrap();
            working.clear();
            self.watches[p].drain(..).for_each(|x| working.push(x));
//            std::mem::swap( working, &mut self.watches[p]);

            let n = working.len();
            for i in 0..n {
                if !self.propagate_clause(working[i], p) {
                    // clause violated
                    // restore remaining watches
                    for j in i + 1..n {
                        self.watches[p].push(working[j]);
                    }
                    self.propagation_queue.clear();
                    self.check_invariants();
                    return Some(working[i]);
                }
            }
        }
        self.check_invariants();
        return None;
    }

    fn propagate_clause(&mut self, clause_id: ClauseId, p: Lit) -> bool {
        let lits = &mut self.clauses[clause_id].disjuncts;
        if lits[0] == !p {
            lits.swap(0, 1);
        }
        debug_assert!(lits[1] == !p);
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
        trace!("Unit clause {}: {}", clause_id, self.clauses[clause_id]);
        self.watches[p].push(clause_id);
        let first_lit = lits[0];
        return self.enqueue(first_lit, Some(clause_id));
    }
    fn value_of(&self, lit: Lit) -> BVal {
        let var_value = self.assignments.get(lit.variable());
        if lit.is_positive() {
            var_value
        } else {
            var_value.neg()
        }
    }
    fn is_set(&self, lit: Lit) -> bool {
        match self.assignments.get(lit.variable()) {
            BVal::Undef => false,
            BVal::True => lit.is_positive(),
            BVal::False => lit.is_negative(),
        }
    }
    pub fn enqueue(&mut self, lit: Lit, reason: Option<ClauseId>) -> bool {
        if let Some(r) = reason {
            // check that the clause does imply the literal
            debug_assert!(self.clauses[r].disjuncts.iter().all(|&l| self.is_set(!l) || l == lit));
        }
        if self.is_set(!lit) {
            // contradiction
            false // implementation in minisat
        } else if self.is_set(lit) {
            // already known
            true
        } else {
            trace!("enqueued: {}", lit);
            self.assignments
                .set(lit.variable(), lit.is_positive(), reason);
            self.propagation_queue.push(lit);
            //            self.check_invariants();
            true
        }
    }

    fn analyze(&self, original_clause: ClauseId) -> (Vec<Lit>, DecisionLevel) {
        // TODO: many allocations to optimize here
        let mut seen = vec![false; self.num_vars as usize + 1]; // todo: use a bitvector
        let mut counter = 0;
        let mut p = None;
        let mut p_reason = Vec::new();
        let mut out_learnt = Vec::new();
        let mut out_btlevel = GROUND_LEVEL;

        {   // some sanity check
            let analyzed = &self.clauses[original_clause].disjuncts;
            // all variables should be false
            debug_assert!(analyzed.iter().all(|&lit| self.value_of(lit) == BVal::False));
            // at least one variable should have been set at the current level
            debug_assert!(analyzed.iter().any(|&lit| self.assignments.level(lit.variable()) == self.assignments.decision_level()));
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
                        debug_assert!((0..simulated_undone).all(|i| self.assignments.last_assignment(i).variable() != qvar));
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
                debug_assert!(
                    self.assignments.level(l.variable()) == self.assignments.decision_level()
                );
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

    fn calc_reason(&self, clause: ClauseId, op: Option<Lit>, out_reason: &mut Vec<Lit>) {
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
        // TODO : bump activity if learnt
    }

    fn backtrack(&mut self) -> Option<Decision> {
        let h = &mut self.heuristic;
        self.assignments.backtrack(&mut |v| h.var_insert(v))
    }

    fn backtrack_to(&mut self, lvl: DecisionLevel) -> Option<Decision> {
        let h = &mut self.heuristic;
        self.assignments.backtrack_to(lvl, &mut |v| h.var_insert(v))
    }

    fn handle_conflict(&mut self, conflict: ClauseId, use_learning: bool) -> SearchStatus {
        self.stats.conflicts += 1;
        self.search_state.conflicts_since_restart += 1;

        if self.assignments.decision_level() == self.assignments.root_level() {
            self.search_state.status = SearchStatus::Unsolvable;
        } else {
            if use_learning {
                let (learnt_clause, backtrack_level) = self.analyze(conflict);
                debug_assert!(backtrack_level < self.assignments.decision_level());
                match self.backtrack_to(backtrack_level) {
                    Some(dec) => trace!("backtracking: {:?}", !dec),
                    None => {
                        self.search_state.status = Unsolvable;
                        return SearchStatus::Unsolvable;
                    }, // no decision left to undo
                }
                let added_clause = self.add_clause(&learnt_clause[..], true);

                match added_clause {
                    AddClauseRes::Inconsistent =>
                        self.search_state.status = Unsolvable,
                    AddClauseRes::Unit(l) => {
                        debug_assert!(learnt_clause[0] == l);
                        // TODO : should backtrack to root level for this to be permanently considered
                        self.enqueue(l, None);
                        self.search_state.status = SearchStatus::Ongoing;
                    }
                    AddClauseRes::Complete(cl_id) => {
                        debug_assert!(learnt_clause[0] == self.clauses[cl_id].disjuncts[0]);
                        self.enqueue(learnt_clause[0], Some(cl_id));
                        self.search_state.status = SearchStatus::Ongoing
                    }
                }
            } else {
                // no learning
                match self.backtrack() {
                    Some(dec) => {
                        trace!("backtracking: {:?}", !dec);
                        self.assume(!dec, None);
                        self.search_state.status = SearchStatus::Ongoing;
                    }
                    None => {
                         self.search_state.status = SearchStatus::Unsolvable; // no decision left to undo
                    }
                }
            }
        }
        return self.search_state.status;
    }

    /// Return None if no solution was found within the conflict limit.
    ///
    fn search(
        &mut self,
    ) -> SearchStatus {

        let mut working_clauses = Vec::new();

        loop {
            match self.propagate(&mut working_clauses) {
                Some(conflict) => {


                    match self.handle_conflict(conflict, self.params.use_learning) {
                        SearchStatus::Unsolvable => return SearchStatus::Unsolvable,
                        SearchStatus::Ongoing => (),
                        _ => unreachable!()
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
                        return SearchStatus::Solution;
                    } else if self.search_state.conflicts_since_restart > self.search_state.allowed_conflicts as usize {
                        // reached bound on number of conflicts
                        // cancel until root level
                        self.backtrack_to(self.assignments.root_level());
                        return SearchStatus::Restarted;
                    } else {
                        let next: BVar = loop {
                            match self.heuristic.next_var() {
                                Some(v) if !self.assignments.is_set(v) => break v, // // not set, select for decision
                                Some(_) => continue, // var already set, proceed to next
                                None => panic!("No unbound value in the heap."),
                            }
                        };

                        self.decide(Decision::True(next));
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

    pub fn solve(&mut self) -> SearchStatus {
        match self.search_state.status {
            SearchStatus::Init => {
                self.search_state.allowed_conflicts = self.params.init_nof_conflict as f64;
                self.search_state.allowed_learnt = self.clauses.num_clauses() as f64 * self.params.init_learnt_ratio;
                self.stats.init_time = time::precise_time_s();
                self.search_state.status = SearchStatus::Ongoing
            },
            SearchStatus::Unsolvable => {
                return SearchStatus::Unsolvable
            },
            SearchStatus::Ongoing | Restarted => {
                // will keep going
            },
            SearchStatus::Solution => {
                // already at a solution, exit immediately
                return Solution;
            }
        }

        loop {
            info!("learnt: {}", self.clauses.num_learnt());
            self.search_state.status = self.search();
            self.stats.end_time = time::precise_time_s();
            match self.search_state.status {
                SearchStatus::Solution => {
                    debug_assert!(self.is_model_valid());
                    return SearchStatus::Solution;
                }
                SearchStatus::Restarted => {
                    // no decision made within bounds
                    self.search_state.allowed_conflicts *= 1.5;
                    self.search_state.allowed_learnt *= 1.1;
                    self.stats.restarts += 1;
                },
                SearchStatus::Unsolvable =>
                    return SearchStatus::Unsolvable,
                _ => unreachable!()
            }
        }
    }

    pub fn integrate_clause(&mut self, mut learnt_clause: Vec<Lit>) -> SearchStatus {
        // we currently only support a conflicting clause
        debug_assert!(learnt_clause.iter().all(|&lit| self.value_of(lit) == BVal::False));
        // sort literals in the clause by descending assignment level
        // this ensure that when backtracking the first two literals (that are watched) will be unset first
        learnt_clause.sort_by_key(|&lit| self.assignments.level(lit.variable()));
        learnt_clause.reverse();

        // get highest decision level of literals in the clause
        let lvl = learnt_clause.iter().map(|&lit| self.assignments.level(lit.variable())).max().unwrap_or(DecisionLevel::ground());
        debug_assert!(lvl <= self.assignments.decision_level());

        match self.add_clause(learnt_clause.as_slice(), false) {
            AddClauseRes::Complete(cl_id) => {
                if lvl < self.assignments.decision_level() {
                    // todo : adapt backtrack_to to support being a no-op
                    self.backtrack_to(lvl);
                }
                match self.handle_conflict(cl_id, true) {
                    SearchStatus::Unsolvable => {
                        self.search_state.status = SearchStatus::Unsolvable;
                        return SearchStatus::Unsolvable
                    },
                    _ => ()
                }
            },
            AddClauseRes::Unit(lit) => {
                // todo : use root_level instead of ground
                if lvl > DecisionLevel::ground() {
                    // todo : adapt backtrack_to to support being a no-op
                    self.backtrack_to(DecisionLevel::ground());
                    self.search_state.status = Restarted;
                }
                if !self.enqueue(lit, None) {
                    self.search_state.status = Unsolvable;
                }
            },
            AddClauseRes::Inconsistent => {
                self.search_state.status = Unsolvable;
            }
        }

        return self.search_state.status;
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
                trace!("Invalid clause: {}: {}", cl_id, self.clauses[cl_id]);
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



#[cfg(test)]
mod tests {
    use super::*;

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
}
