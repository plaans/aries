use std::collections::{BTreeMap, BTreeSet};

use aries::{
    backtrack::Backtrack,
    core::{IntCst, Lit},
    model::lang::{IVar, hreif::Store, linear::LinearSum},
    solver::{Solver, musmcs::MusMcs},
};
use itertools::Itertools;

use crate::{ConstraintID, Sched};

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
        let mut encoding = sched.model.clone();

        let mut assumptions_map = BTreeMap::new();
        let mut trigger = BTreeMap::new();

        for (cid, c) in sched.constraints.iter().enumerate() {
            if let Some(tag) = project(cid) {
                let l = if let Some(l) = trigger.get(&tag) {
                    *l
                } else {
                    let l = encoding.new_literal(Lit::TRUE);
                    assumptions_map.insert(l, tag.clone());
                    trigger.insert(tag, l);
                    l
                };
                c.enforce_if(l, sched, &mut encoding);
            } else {
                c.enforce(sched, &mut encoding);
            }
        }
        let solver = Solver::new(encoding);
        Self {
            solver,
            enablers: assumptions_map,
        }
    }

    /// Returns true if the model is satisfiable with all assumptions
    pub fn check_satisfiability(&mut self) -> bool {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let res = self.solver.solve_with_assumptions(&assumptions).unwrap().is_ok();
        self.solver.print_stats();
        self.solver.reset(); // TODO: this should not be needed
        res
    }

    /// Returns an iterator over all MUS and MCS in the model.
    pub fn explain_unsat<'x>(&'x mut self) -> impl Iterator<Item = MusMcs<T>> + 'x {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let projection = |l: &Lit| self.enablers.get(l).cloned();
        self.solver
            .mus_and_mcs_enumerator(&assumptions)
            .map(move |mm| mm.project(projection))
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
            .fold(LinearSum::constant_int(num_assumptions), |sum, l| {
                sum - IVar::new(l.variable())
            });
        self.solver
            .enforce(num_relaxed_assumptions.leq(max_relaxed_assumptions), []);

        for allowed_relaxations in 0..num_assumptions {
            println!("Current lower bound: {}", allowed_relaxations);
            let result = self
                .solver
                .solve_with_assumptions(&[max_relaxed_assumptions.leq(allowed_relaxations)])
                .unwrap();
            if let Ok(sol) = result {
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
            self.solver.reset();
        }
        None
    }
}
