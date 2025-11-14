use std::collections::{BTreeMap, BTreeSet};

use aries::{
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

    pub fn explain_unsat<'x>(&'x mut self) -> impl Iterator<Item = MusMcs<T>> + 'x {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let projection = |l: &Lit| self.enablers.get(l).cloned();
        self.solver
            .mus_and_mcs_enumerator(&assumptions)
            .map(move |mm| mm.project(projection))
    }

    pub fn find_smallest_mcs(&mut self) -> BTreeSet<T> {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let num_assumptions = assumptions.len() as IntCst;
        let bound = self.solver.model.new_ivar(0, assumptions.len() as IntCst, "objective");
        let sum = assumptions
            .iter()
            .fold(LinearSum::zero(), |sum, l| sum + IVar::new(l.variable()));
        self.solver.enforce(sum.geq(bound), []);
        if let Some((obj, sol)) = self
            .solver
            .maximize_with_callback(bound, |new_boj, _| println!("new CS: {}", num_assumptions - new_boj))
            .unwrap()
        {
            self.solver.print_stats();
            println!(
                "OPTIMAL: {obj} / {}   :   {}",
                assumptions.len(),
                (assumptions.len() as IntCst) - obj
            );
            assumptions
                .into_iter()
                .filter(|l| sol.entails(!*l))
                .map(|l| self.enablers[&l].clone())
                .collect()
        } else {
            panic!("UNSAT!")
        }
    }
}
