pub mod collection;
mod core;
//use crate::collection::index_map::*;
use crate::collection::index_map::*;
use crate::core::all::*;
use std::ops::{Not, RangeInclusive};

use log::{debug, info, trace};

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

pub struct SearchParams {
    var_decay: f64,
    cla_decay: f64,
    init_nof_conflict: usize,
    init_learnt_ratio: f64,
}
impl SearchParams {
    pub fn defaults() -> Self {
        SearchParams {
            var_decay: 0.95,
            cla_decay: 0.999,
            init_nof_conflict: 100,
            init_learnt_ratio: 1_f64 / 3_f64,
        }
    }
}

pub struct Solver {
    num_vars: u32,
    assignments: Assignments,
    clauses: Vec<Clause>, // this is ClauseId -> Clause
    watches: IndexMap<Lit, Vec<ClauseId>>,
    propagation_queue: Vec<Lit>,
}

impl Solver {
    pub fn init(clauses: Vec<Clause>) -> Self {
        let mut biggest_var = 0;
        for cl in &clauses {
            for lit in &cl.disjuncts {
                biggest_var = biggest_var.max(lit.variable().id.get())
            }
        }

        let mut watches = IndexMap::new_with(((biggest_var + 1) * 2) as usize, || Vec::new());
        for i in 0..clauses.len() {
            let cl = &clauses[i];
            let cl_id = ClauseId(i);
            assert!(cl.disjuncts.len() >= 2);
            watches[!cl.disjuncts[0]].push(cl_id);
            watches[!cl.disjuncts[1]].push(cl_id);
        }

        let solver = Solver {
            num_vars: biggest_var,
            assignments: Assignments::new(biggest_var),
            clauses,
            watches,
            propagation_queue: Vec::new(),
        };
        solver.check_invariants();
        solver
    }

    pub fn variables(&self) -> RangeInclusive<BVar> {
        RangeInclusive::new(BVar::from_bits(1), BVar::from_bits(self.num_vars))
    }

    pub fn decide(&mut self, dec: Decision) {
        self.check_invariants();
        trace!("decision: {:?}", dec);
        self.assignments.add_backtrack_point(dec);
        self.assume(dec);
    }
    pub fn assume(&mut self, dec: Decision) {
        self.check_invariants();
        match dec {
            Decision::True(var) => {
                self.assignments.set(var, true);
                self.propagation_queue.push(var.lit(true));
            }
            Decision::False(var) => {
                self.assignments.set(var, false);
                self.propagation_queue.push(var.lit(false));
            }
        }
        self.check_invariants();
    }

    /// Returns:
    ///   Some(i): in case of a conflict where i is the id of the violated clause
    ///   None if no conflict was detected during propagation
    pub fn propagate(&mut self) -> Option<ClauseId> {
        self.check_invariants();
        while !self.propagation_queue.is_empty() {
            let p = self.propagation_queue.pop().unwrap();

            let todo = self.watches[p].clone();
            self.watches[p].clear();
            let n = todo.len();
            for i in 0..n {
                if !self.propagate_clause(todo[i], p) {
                    // clause violated
                    // restore remaining watches
                    for j in i + 1..n {
                        self.watches[p].push(todo[j]);
                    }
                    self.propagation_queue.clear();
                    self.check_invariants();
                    return Some(todo[i]);
                }
            }
        }
        self.check_invariants();
        return None;
    }

    fn propagate_clause(&mut self, clause_id: ClauseId, p: Lit) -> bool {
        let lits = &mut self.clauses[clause_id.0].disjuncts;
        if lits[0] == !p {
            lits.swap(0, 1);
        }
        debug_assert!(lits[1] == !p);
        let lits = &self.clauses[clause_id.0].disjuncts;
        if self.is_set(lits[0]) {
            // clause satisfied, restore the watch and exit
            self.watches[p].push(clause_id);
            //            self.check_invariants();
            return true;
        }
        for i in 2..lits.len() {
            if !self.is_set(!lits[i]) {
                let lits = &mut self.clauses[clause_id.0].disjuncts;
                lits.swap(1, i);
                self.watches[!lits[1]].push(clause_id);
                //                self.check_invariants();
                return true;
            }
        }
        // no replacement found, clause is unit
        trace!("Unit clause {}: {}", clause_id.0, self.clauses[clause_id.0]);
        self.watches[p].push(clause_id);
        //        self.check_invariants();
        return self.enqueue(lits[0]);
    }
    fn is_set(&self, lit: Lit) -> bool {
        match self.assignments.get(lit.variable()) {
            BVal::Undef => false,
            BVal::True => lit.is_positive(),
            BVal::False => lit.is_negative(),
        }
    }
    pub fn enqueue(&mut self, lit: Lit) -> bool {
        if self.is_set(!lit) {
            // contradiction
            false
        } else if self.is_set(lit) {
            // already known
            true
        } else {
            trace!("enqueued: {}", lit);
            self.assignments
                .set(lit.variable(), if lit.is_negative() { false } else { true });
            self.propagation_queue.push(lit);
            //            self.check_invariants();
            true
        }
    }

    fn analyze(&self, original_clause: ClauseId) -> (Vec<Lit>, DecisionLevel) {
        let mut seen = vec![false; self.num_vars as usize + 1]; // todo: use a bitvector
        let mut counter = 0;
        let mut p = None;
        let mut p_reason = Vec::new();
        let mut out_learnt = Vec::new();
        let mut out_btlevel = GROUND_LEVEL;

        let mut clause = original_clause;
        let mut simulated_undone = 0;

        out_learnt.push(Lit::dummy());

        let mut first = true;
        while first || counter > 0 {
            first = false;
            p_reason.clear();
            self.calc_reason(clause, p, &mut p_reason);

            for &q in &p_reason {
                let qvar = q.variable();
                if !seen[q.variable().to_index()] {
                    seen[q.variable().to_index()] = true;
                    if self.assignments.level(qvar) == self.assignments.decision_level() {
                        counter += 1;
                    } else if self.assignments.level(qvar) > GROUND_LEVEL {
                        out_learnt.push(!q);
                        out_btlevel = out_btlevel.max(self.assignments.level(qvar));
                    }
                }
            }

            loop {
                p = Some(self.assignments.last_assignment(simulated_undone));
                clause = self.reason(p.unwrap().variable()).unwrap();
                simulated_undone += 1;
                //                p = self.assignments
                if !seen[p.unwrap().variable().to_index()] {
                    break;
                }
            }
            counter -= 1;
        }
        debug_assert!(out_learnt[0] == Lit::dummy());
        out_learnt[0] = !p.unwrap();

        let x = Clause {
            disjuncts: out_learnt.clone(),
        };
        println!("{}", x);
        (out_learnt, out_btlevel)
    }

    fn calc_reason(&self, clause: ClauseId, op: Option<Lit>, out_reason: &mut Vec<Lit>) {
        let cl = &self.clauses[clause.0].disjuncts;
        debug_assert!(out_reason.is_empty());
        debug_assert!(op.iter().all(|&p| cl[0] == p));
        let first = match op {
            Some(_) => 1,
            None => 0,
        };
        for &l in &cl[first..] {
            out_reason.push(!l);
        }
        // TODO : bump activity if learnt
    }

    fn reason(&self, var: BVar) -> Option<ClauseId> {
        unimplemented!()
    }

    /// Return None if no solution was found within the conflict limit.
    ///
    pub fn search(
        &mut self,
        nof_conflicts: usize,
        nof_learnt: usize,
        params: &SearchParams,
    ) -> Option<bool> {
        debug_assert!(self.assignments.decision_level() == self.assignments.root_level());

        let var_decay = 1_f64 / params.var_decay;
        let cla_decay = 1_f64 / params.cla_decay;

        let mut conflict_count: usize = 0;

        loop {
            match self.propagate() {
                Some(conflict) => {
                    //conflict_count += 1;
                    /*

                    if self.decision_level() == self.root_level() {
                        unimplemented!()
                    } else {
                        let (learnt_clause, backtrack_level) = self.analyze(conflict);
                        // cancel until
                        // record clause
                        // decay activities
                    }*/
                    match self.assignments.backtrack() {
                        Some(dec) => {
                            trace!("backtracking: {:?}", !dec);
                            self.assume(!dec);
                        }
                        None => {
                            return Some(false); // no decision left to undo
                        }
                    }
                }
                None => {
                    if self.assignments.decision_level() == GROUND_LEVEL {
                        // TODO: simplify db
                    }
                    if self.num_learnt() as i64 - self.assignments.num_assigned() as i64
                        >= nof_learnt as i64
                    {
                        // TODO: reduce learnt set
                    }

                    if self.num_vars() as usize == self.assignments.num_assigned() {
                        // model found
                        return Some(true);
                    } else if conflict_count > nof_conflicts {
                        // reached bound on number of conflicts
                        // cancel until root level
                        // TODO: force a restart
                        println!("Restart");
                        return None;
                    } else {
                        let mut v = *self.variables().start();
                        let last = *self.variables().end();
                        while v <= last {
                            if !self.assignments.is_set(v) {
                                self.decide(Decision::True(v));
                                break;
                            }
                            v = v.next();
                        }
                        // select var
                    }
                }
            }
        }
    }
    fn num_vars(&self) -> u32 {
        self.num_vars
    }
    fn num_learnt(&self) -> usize {
        //TODO
        0
    }

    pub fn solve(&mut self, params: &SearchParams) -> bool {
        let mut nof_conflicts = params.init_nof_conflict as f64;
        let mut nof_learnt = self.clauses.len() as f64 / params.init_learnt_ratio;

        loop {
            match self.search(nof_conflicts as usize, nof_learnt as usize, params) {
                Some(is_sat) => {
                    // TODO: restore state
                    return is_sat;
                }
                None => {
                    // no decision made within bounds
                    nof_conflicts *= 1.5;
                    nof_learnt *= 1.1;
                }
            }
        }
    }

    fn is_model_valid(&self) -> bool {
        self.check_invariants();
        let mut i = 0;
        for cl in &self.clauses {
            let mut is_sat = false;
            for lit in &cl.disjuncts {
                if self.is_set(*lit) {
                    is_sat = true;
                }
            }
            if !is_sat {
                trace!("Invalid clause: {}: {}", i, cl);
                return false;
            }
            i += 1;
        }
        true
    }

    #[cfg(not(feature = "full_check"))]
    fn check_invariants(&self) {}

    #[cfg(feature = "full_check")]
    fn check_invariants(&self) {
        let num_clauses = self.clauses.len();
        let mut watch_count = IndexMap::new(num_clauses, 0);
        for watches_for_lit in &self.watches.values[1..] {
            for watcher in watches_for_lit {
                watch_count[*watcher] += 1;
            }
        }
        assert!(watch_count.values.iter().all(|&n| n == 2))
    }
}

use env_logger::Target;
use log::LevelFilter;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    file: String,
    #[structopt(long = "sat")]
    expected_satifiability: Option<bool>,
    #[structopt(short = "v")]
    verbose: bool,
}

fn main() {
    let opt = Opt::from_args();
    env_logger::builder()
        .filter_level(if opt.verbose {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        })
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .target(Target::Stdout)
        .init();

    log::debug!("Options: {:?}", opt);

    let filecontent = fs::read_to_string(opt.file).expect("Cannot read file");

    let clauses = core::cnf::CNF::parse(&filecontent).clauses;

    let mut solver = Solver::init(clauses);
    let vars = solver.variables();
    let sat = solver.solve(&SearchParams::defaults());
    match sat {
        true => {
            assert!(solver.is_model_valid());

            info!("==== Model found ====");

            let mut v = *vars.start();
            while v <= *vars.end() {
                debug!("{} <- {:?}", v.to_index(), solver.assignments.get(v));
                v = v.next();
            }
            if opt.expected_satifiability == Some(false) {
                eprintln!("Error: expected UNSAT but got SAT");
                std::process::exit(1);
            }
        }
        false => {
            info!("Unsat");

            if opt.expected_satifiability == Some(true) {
                eprintln!("Error: expected SAT but got UNSAT");
                std::process::exit(1);
            }
        }
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
