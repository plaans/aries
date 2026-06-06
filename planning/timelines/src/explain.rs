use std::collections::{BTreeMap, BTreeSet};

use aries_solver::prelude::*;
use aries_solver::{
    backtrack::Backtrack,
    lang::*,
    solver::{Solver, musmcs::MusMcs},
};
use itertools::Itertools;

use crate::{ConstraintID, IntExp, Sched};

pub struct ExplainableSolver<T> {
    solver: Solver<crate::Sym>,
    enablers: BTreeMap<Lit, T>,
}

impl<T: Ord + Clone> ExplainableSolver<T> {
    /// Creates a new explainability oriented solver where constraints are partitioned into:
    ///
    ///  - background constraints (strong), for which the projection returns `None`
    ///  - foreground constraints (soft), enabled by assumptions. Two foregrounds constriants with the
    ///    same projection will be enabled together by the same assumption.
    pub fn new(sched: &Sched, project: impl Fn(ConstraintID) -> Option<T>) -> Self {
        let mut encoding = sched.clone().encoder();

        let mut assumptions_map = BTreeMap::new();
        let mut trigger = BTreeMap::new();

        for (cid, c) in sched.constraints.iter().enumerate() {
            tracing::debug!("Adding constraint: {c:?}");
            if let Some(tag) = project(cid) {
                let l = if let Some(l) = trigger.get(&tag) {
                    *l
                } else {
                    let l = encoding.new_literal(Lit::TRUE); // could we use the conjunctive scope directly?
                    assumptions_map.insert(l, tag.clone());
                    trigger.insert(tag, l);
                    l
                };
                c.opt_enforce_if(l, &mut encoding);
            } else {
                c.enforce(&mut encoding);
            }
        }
        let solver = Solver::new(encoding.store);
        Self {
            solver,
            enablers: assumptions_map,
        }
    }

    /// Check if the model is satifiable with all assumptions, and returns a solution if it is.
    pub fn check_satisfiability(&mut self) -> Option<Solution> {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let res = self
            .solver
            .solve_with_assumptions(&assumptions, aries_solver::solver::SearchLimit::None)
            .unwrap()
            .ok();
        // self.solver.print_stats();
        self.solver.reset(); // TODO: this should not be needed
        res
    }

    /// Find an optimal solution with all assumptions enforced.
    /// The method accepts an additional set of assumptions that will be enforced in all solutions.
    pub fn find_optimal(
        &mut self,
        obj: LinTerm,
        on_new_solution: impl FnMut(&Solution),
        under_assumptions: impl Into<Vec<Lit>>,
    ) -> Option<Solution> {
        let mut assumptions = under_assumptions.into();
        // add assumptions for detecting unsatifable constraints
        for &enabler in self.enablers.keys() {
            assumptions.push(enabler);
        }
        let res = self
            .solver
            .minimize_with_assumptions(
                obj,
                &assumptions,
                aries_solver::solver::SearchLimit::None,
                on_new_solution,
            )
            .unwrap();
        // self.solver.print_stats();
        self.solver.reset(); // TODO: this should not be needed
        res.ok()
    }

    /// Returns an iterator over all MUS and MCS in the model.
    pub fn explain_unsat<'x>(&'x mut self) -> impl Iterator<Item = MusMcs<T>> + 'x {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let projection = |l: &Lit| self.enablers.get(l).cloned();
        self.solver
            .mus_and_mcs_enumerator(&assumptions)
            .map(move |mm| mm.project(projection))
    }

    /// Returns an iterator over all MUS (Minimal Unsatifiable Subsets) in the model.
    pub fn muses(&mut self) -> impl Iterator<Item = BTreeSet<T>> + '_ {
        self.explain_unsat().filter_map(|mus_mcs| match mus_mcs {
            MusMcs::Mus(mus) => Some(mus),
            MusMcs::Mcs(_) => None,
        })
    }

    /// Returns the smallest MCS over all assumptions
    pub fn find_smallest_mcs(&mut self) -> Option<BTreeSet<T>> {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let num_assumptions = assumptions.len() as IntCst;

        // create a variable that requires a maximal number of assumptions to hold
        // ` |{ass | holds(ass), for ass in assumptions}| <= bound`
        let max_relaxed_assumptions = self.solver.model.new_ivar(0, assumptions.len() as IntCst, "objective");
        let num_relaxed_assumptions = assumptions
            .iter()
            .fold(IntExp::cst(num_assumptions), |sum, l| sum - l.variable());
        self.solver
            .enforce(num_relaxed_assumptions.leq(max_relaxed_assumptions), []);

        for allowed_relaxations in 0..num_assumptions {
            println!("Current lower bound: {}", allowed_relaxations);
            let result = self
                .solver
                .solve_with_assumptions(
                    &[max_relaxed_assumptions.leq(allowed_relaxations)],
                    aries_solver::solver::SearchLimit::None,
                )
                .unwrap();
            match result {
                Ok(sol) => {
                    self.solver.print_stats();
                    println!("OPTIMAL: {allowed_relaxations} / {} ", num_assumptions);
                    return Some(
                        assumptions
                            .into_iter()
                            .filter(|l| sol.entails(!*l))
                            .map(|l| self.enablers[&l].clone())
                            .collect(),
                    );
                }
                Err(core) if core.literals().is_empty() => {
                    // UNSAT with no assumptions to relax
                    return None;
                }
                Err(_core) => {
                    // assumption makes problem unsat, relax in next round
                }
            };
            self.solver.reset();
        }
        None
    }
}
