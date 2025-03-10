use std::collections::BTreeSet;
use std::sync::Arc;

use aries::backtrack::Backtrack;
use aries::core::Lit;
use aries::model::Label;
use aries::solver::{Exit, Solver, UnsatCore};

pub struct SubsetSolver<Lbl: Label> {
    pub solver: Solver<Lbl>,
    soft_constraints_reifications: Arc<BTreeSet<Lit>>,
    /// The latest unsat core computed in `check_seed_sat` (in the `false` return case).
    cached_unsat_core: BTreeSet<Lit>,
    /// Used for an optimization. Set of (soft constraint reification) literals
    /// that have been found to constitute a singleton MCS, i.e. belonging to all MUSes.
    pub necessarily_in_all_muses: BTreeSet<Lit>,
}

impl<Lbl: Label> SubsetSolver<Lbl> {
    pub fn new(
        solver: Solver<Lbl>,
        soft_constraints_reifications: Arc<BTreeSet<Lit>>,
    ) -> Self {
        SubsetSolver::<Lbl> {
            solver,
            soft_constraints_reifications,
            cached_unsat_core: BTreeSet::new(),
            necessarily_in_all_muses: BTreeSet::new(),
        }
    }

    pub fn check_seed_sat(
        &mut self,
        seed: &BTreeSet<Lit>,
        solve_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
    ) -> Result<bool, Exit> {
        // FIXME warm-start / solution hints optimization should go here... right ?
        // TODO: either use "manual" warm starting (save the assignment when sat case, and reuse it as a starting point for the next call)
        //       or leverage incrementality and, when the case is sat, 
        let res = solve_fn(&mut self.solver, &seed)?;
        self.solver.reset(); // TODO (as part of warm-start / leveraging incrementality: simply reset_search ? (not the full solver!..?))

        if let Err(unsat_core) = res {
            self.cached_unsat_core = unsat_core
                .literals()
                .into_iter()
                .chain(&self.necessarily_in_all_muses)
                .copied()
                .collect();
            Ok(false)
        } else {
            Ok(true)
        }
    }

    pub fn grow(
        &mut self,
        seed: &BTreeSet<Lit>,
        solve_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
    ) -> Result<(BTreeSet<Lit>, Option<BTreeSet<Lit>>), Exit> {
        let mut mss = seed.clone();
        for &lit in self.soft_constraints_reifications.clone().difference(seed) {
            mss.insert(lit);
            if !self.check_seed_sat(&mss, &solve_fn)? {
                mss.remove(&lit);
            }
        }
        let mcs: BTreeSet<Lit> = self.soft_constraints_reifications.difference(&mss).copied().collect();

        // If the found correction set only has 1 element,
        // then that element is added to those that are known to be in all unsatisfiable sets.
        if mcs.len() == 1 {
            self.necessarily_in_all_muses.insert(mcs.first().unwrap().clone());
        }
        Ok((mss, Some(mcs)))
    }

    pub fn shrink(
        &mut self,
        seed: &BTreeSet<Lit>,
        solve_fn: impl Fn(&mut Solver<Lbl>, &BTreeSet<Lit>) -> Result<Result<(), UnsatCore>, Exit>,
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
            if !self.check_seed_sat(&mus, &solve_fn)? {
                mus = self.cached_unsat_core.clone();
            } else {
                debug_assert!(!mus.contains(&lit));
                mus.insert(lit);
            }
        }
        Ok(mus)
    }
}