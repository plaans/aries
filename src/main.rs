pub mod collection;
mod core;
//use crate::collection::index_map::*;
use crate::collection::index_map::*;
use crate::core::all::*;
use std::ops::RangeInclusive;

use log::{debug, info, trace};

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

    pub fn decide(&mut self, var: BVar, val: bool) {
        self.check_invariants();
        trace!("decision: {:?} <- {}", var, val);
        debug_assert!(!self.assignments.is_set(var));
        self.assignments.add_backtrack_point();
        self.assignments.set(var, BVal::from_bool(val));
        self.propagation_queue.push(var.lit(val));
        self.check_invariants();
    }

    pub fn propagate(&mut self) -> bool {
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
                    return false;
                }
            }
        }
        self.check_invariants();
        true
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
        trace!("Unit clause: {}", clause_id.0);
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
            trace!("enqueued: {:?} <- {}", lit.variable(), lit.is_positive());
            self.assignments.set(
                lit.variable(),
                if lit.is_negative() {
                    BVal::False
                } else {
                    BVal::True
                },
            );
            self.propagation_queue.push(lit);
            //            self.check_invariants();
            true
        }
    }

    pub fn solve(&mut self) -> bool {
        let first_var = BVar::from_bits(1);
        let last_var = BVar::from_bits(self.num_vars);

        struct Decision(BVar, bool);
        let mut decisions: Vec<Decision> = Vec::new();

        decisions.push(Decision(first_var, false));
        decisions.push(Decision(first_var, true));

        loop {
            match decisions.pop() {
                Some(Decision(var, val)) => {
                    while self.assignments.is_set(var) {
                        self.assignments.backtrack()
                    }
                    self.decide(var, val);
                    if self.propagate() {
                        // consistent
                        let next = {
                            let mut tmp =
                                BVar::from_bits(decisions.last().map_or(1, |d| d.0.id.get()));
                            while tmp <= last_var && self.assignments.is_set(tmp) {
                                tmp = BVar::from_bits(tmp.id.get() + 1)
                            }
                            tmp
                        };
                        if next > last_var {
                            //                            break; // all vars assigned
                            return true;
                        } else {
                            decisions.push(Decision(next, false));
                            decisions.push(Decision(next, true));
                        }
                    } else {
                        // not consistent
                        // this will backtrack
                        trace!("INCONSISTENCY");
                    }
                }
                None => {
                    // first ?
                    // no solutions
                    return false;
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
                trace!("Invalid clause: {}: {:?}", i, cl.disjuncts);
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
    let sat = solver.solve();
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
