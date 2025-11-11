use std::collections::BTreeMap;

use aries::{
    core::Lit,
    model::lang::hreif::Store,
    solver::{Solver, musmcs::MusMcs},
};
use itertools::Itertools;

use crate::{ConstraintID, Sched};

pub struct ExplainableSolver<'a, T> {
    sched: &'a Sched,
    solver: Solver<crate::Sym>,
    enablers: BTreeMap<Lit, T>,
}

impl<'a, T: Ord + Clone> ExplainableSolver<'a, T> {
    /// Creates a new explainability oriented solver where constraints are partitioned into:
    ///
    ///  - background constraints (strong), for which the projection returns `None`
    ///  - foreground constraints (soft), enabled by assumptions. Two foregrounds constriants with the
    ///    same projection will be enabled together by the same assumption.
    pub fn new(sched: &'a Sched, project: impl Fn(ConstraintID) -> Option<T>) -> Self {
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
            sched,
            solver,
            enablers: assumptions_map,
        }
    }

    pub fn explain_unsat<'x>(&'x mut self) -> impl Iterator<Item = MusMcs<T>> + 'x {
        let assumptions = self.enablers.keys().copied().collect_vec();
        let projection = |l: &Lit| {
            if let Some(cid) = self.enablers.get(l) {
                Some(cid.clone())
            } else {
                None
            }
        };
        self.solver
            .mus_and_mcs_enumerator(&assumptions)
            .map(move |mm| mm.project(projection))
    }
}
