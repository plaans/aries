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

        // create a variable that requires a minimal number of assumptions to hold
        // `bound <= |{ass | holds(ass), for ass in assumptions}|
        let min_holding_assumptions = self.solver.model.new_ivar(0, assumptions.len() as IntCst, "objective");
        let num_holding_assumptions = assumptions
            .iter()
            .fold(LinearSum::zero(), |sum, l| sum + IVar::new(l.variable()));
        self.solver
            .enforce(num_holding_assumptions.geq(min_holding_assumptions), []);

        // maximize the lower bound, the solution will have a globally minimal set of assumptions violated
        // these assumptions are a smallest MCS
        if let Some((obj, sol)) = self
            .solver
            .maximize_with_callback(min_holding_assumptions, |new_objective, _| {
                println!("new CS: {}", num_assumptions - new_objective)
            })
            .unwrap()
        {
            self.solver.print_stats();
            println!("OPTIMAL: {obj} / {}   :   {}", num_assumptions, (num_assumptions) - obj);

            // identify the assumptions that do not hold in the solution
            Some(
                assumptions
                    .into_iter()
                    .filter(|l| sol.entails(!*l))
                    .map(|l| self.enablers[&l].clone())
                    .collect(),
            )
        } else {
            // problem is unsat (even when relaxing all assumptions)
            None
        }
    }
}
