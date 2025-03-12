use std::collections::BTreeSet;
use std::sync::Arc;

use aries::core::Lit;
use aries::model::Label;
use aries::solver::{Exit, Solver, UnsatCore};

pub struct SubsetSolver<Lbl: Label> {
    pub solver: Solver<Lbl>,
    soft_constraints_reifications: Arc<BTreeSet<Lit>>,
    /// Used for an optimization. Set of (soft constraint reification) literals
    /// that have been found to constitute a singleton MCS, i.e. belonging to all MUSes.
    pub necessarily_in_all_muses: BTreeSet<Lit>,
}

impl<Lbl: Label> SubsetSolver<Lbl> {
    pub fn new(solver: Solver<Lbl>, soft_constraints_reifications: Arc<BTreeSet<Lit>>) -> Self {
        SubsetSolver::<Lbl> {
            solver,
            soft_constraints_reifications,
            necessarily_in_all_muses: BTreeSet::new(),
        }
    }

    pub fn check_subset(
        &mut self,
        seed: &BTreeSet<Lit>,
        find_unsat_core_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
    ) -> Result<Result<(), BTreeSet<Lit>>, Exit> {
        let res = find_unsat_core_fn(&mut self.solver, seed)?;
        // NOTE: any resetting (or not!) of the solver is assumed to be done in `solve_fn`
        Ok(res.map_err(|unsat_core| {
            unsat_core
                .literals()
                .iter()
                .chain(&self.necessarily_in_all_muses)
                .copied()
                .collect()
        }))
    }

    pub fn grow(
        &mut self,
        seed: &BTreeSet<Lit>,
        find_unsat_core_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
    ) -> Result<(BTreeSet<Lit>, Option<BTreeSet<Lit>>), Exit> {
        let mut mss = seed.clone();
        for &lit in self.soft_constraints_reifications.clone().difference(seed) {
            mss.insert(lit);
            if self.check_subset(&mss, &find_unsat_core_fn)?.is_err() {
                mss.remove(&lit);
            }
        }
        let mcs: BTreeSet<Lit> = self.soft_constraints_reifications.difference(&mss).copied().collect();

        // If the found correction set only has 1 element,
        // then that element is added to those that are known to be in all unsatisfiable sets.
        if mcs.len() == 1 {
            self.necessarily_in_all_muses.insert(*mcs.first().unwrap());
        }
        Ok((mss, Some(mcs)))
    }

    pub fn shrink(
        &mut self,
        seed: &BTreeSet<Lit>,
        find_unsat_core_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
    ) -> Result<BTreeSet<Lit>, Exit> {
        let mut mus: BTreeSet<Lit> = seed.clone();
        for &lit in seed {
            if !mus.contains(&lit) {
                continue;
            }
            // Optimization: if the literal has been determined to belong to all muses,
            // no need to check if, without it, the set would be satisfiable (because it obviously would be).
            if self.necessarily_in_all_muses.contains(&lit) {
                continue;
            }
            mus.remove(&lit);
            if let Err(unsat_core) = self.check_subset(&mus, &find_unsat_core_fn)? {
                mus = unsat_core;
            } else {
                debug_assert!(!mus.contains(&lit));
                mus.insert(lit);
            }
        }
        Ok(mus)
    }
}
