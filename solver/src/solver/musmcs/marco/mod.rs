mod mapsolver;

use crate::backtrack::Backtrack;
use crate::core::Lit;
use crate::model::Label;
use crate::solver::{Exit, Solver};

use std::{collections::BTreeSet, time::Instant};

use itertools::Itertools;

use crate::solver::musmcs::{Mcs, Mus, MusMcsResult};
use mapsolver::MapSolver;
pub use mapsolver::MapSolverMode;

/// In theory, `KnownImplications` should be strictly better than `KnownSingletonMCSes`,
/// but the additional work needed to find these implications (involves propagations back and forth) could certainly be not worth it.
#[derive(Copy, Clone, Default)]
pub enum SubsetSolverOptiMode {
    None,
    #[default]
    KnownSingletonMCSes,
    KnownImplications,
}

pub struct Marco<'a, Lbl: Label> {
    /// The literals whose powerset makes up the search space of the MARCO.
    literals: BTreeSet<Lit>,
    /// The "subset solver", sometimes also simply called "constraint solver".
    /// To avoid confusion, we will refer to it as the "main solver".
    main_solver: &'a mut Solver<Lbl>,
    map_solver: MapSolver,
}

impl<'a, Lbl: Label> Marco<'a, Lbl> {
    pub fn with(
        literals: impl Iterator<Item = Lit> + Clone,
        mainsolver: &'a mut Solver<Lbl>,
        map_solver_mode: MapSolverMode,
    ) -> Self {
        let mapsolver = MapSolver::new(literals.clone(), map_solver_mode);
        Self {
            literals: literals.into_iter().collect(),
            main_solver: mainsolver,
            map_solver: mapsolver,
        }
    }

    pub fn run(&mut self, on_mus_found: Option<fn(&Mus)>, on_mcs_found: Option<fn(&Mcs)>) -> MusMcsResult {
        let mut muses = Vec::<Mus>::new();
        let mut mcses = Vec::<Mcs>::new();

        let start = Instant::now();

        let complete = self
            ._run(
                &mut muses,
                &mut mcses,
                on_mus_found,
                on_mcs_found,
                SubsetSolverOptiMode::default(),
            )
            .is_ok();

        debug_assert!(muses.iter().all_unique());
        debug_assert!(mcses.iter().all_unique());

        MusMcsResult {
            muses,
            mcses,
            complete: Some(complete),
            run_time: Some(start.elapsed()),
        }
    }

    fn _run(
        &mut self,
        muses: &mut Vec<Mus>,
        mcses: &mut Vec<Mcs>,
        on_mus_found: Option<fn(&Mus)>,
        on_mcs_found: Option<fn(&Mcs)>,
        opti_mode: SubsetSolverOptiMode,
    ) -> Result<(), Exit> {
        while let Some(seed) = self.map_solver.find_unexplored_seed()? {
            if self.check_subset(&seed)?.is_ok() {
                let (_, mcs) = self.grow(&seed, opti_mode)?;
                self.map_solver.block_down(&mcs);

                debug_assert!(mcses.iter().all(|set| !mcs.is_subset(set) && !set.is_subset(&mcs)));
                if !mcs.is_empty() {
                    on_mcs_found.unwrap_or(|_| ())(&mcs);
                    mcses.push(mcs);
                }
            } else {
                // debug_assert!(self.mapsolver.seed_is_unexplored(&seed));

                let mus = self.shrink(&seed, opti_mode)?;
                self.map_solver.block_up(&mus);

                debug_assert!(muses.iter().all(|set| !mus.is_subset(set) && !set.is_subset(&mus)));
                if !mus.is_empty() {
                    on_mus_found.unwrap_or(|_| ())(&mus);
                    muses.push(mus);
                }
            }
        }
        Ok(())
    }

    /// Checks whether the given subset literals is satisfiable.
    /// - If SAT: returns *all* literals (considered by the algorithm) that are true in the found assignment (so a superset of `subset`).
    /// - If UNSAT: returns an unsat core of `subset`.
    fn check_subset(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<BTreeSet<Lit>, BTreeSet<Lit>>, Exit> {
        let mut find_unsat_core_fn = |assumptions: &[Lit]| {
            let res = self.main_solver.solve_with_assumptions(assumptions)?;
            self.main_solver.reset();
            Ok(res)
        };

        let res = match find_unsat_core_fn(&subset.iter().copied().collect_vec())? {
            Ok(assignment) => Ok(self
                .literals
                .iter()
                .filter(|&&l| assignment.entails(l))
                .copied()
                .collect()),
            Err(unsat_core) => Err(unsat_core.literals().iter().copied().collect()),
        };
        Ok(res)
    }

    /// Find a MSS by adding literals to `sat_subset`, until no more can be added without leading to UNSAT.
    ///
    /// Optional optimization may allow skipping satisfiability checks for some additions.
    fn grow(
        &mut self,
        sat_subset: &BTreeSet<Lit>,
        opti_mode: SubsetSolverOptiMode,
    ) -> Result<(BTreeSet<Lit>, Mcs), Exit> {
        let sat_subset_complement = self.literals.difference(sat_subset).copied().collect_vec();
        let mut current = sat_subset.clone();

        let mut skip = BTreeSet::<Lit>::new();
        self.grow_optional_optimisation_lits_to_skip(opti_mode, &current, &mut skip);

        for lit in sat_subset_complement {
            if current.contains(&lit) || skip.contains(&lit) {
                continue;
            }
            current.insert(lit);

            if let Ok(superset) = self.check_subset(&current)? {
                current = superset;
                self.grow_optional_optimisation_lits_to_skip(opti_mode, &current, &mut skip);
            } else {
                current.remove(&lit);
            }
        }
        let mss = current;
        let mcs = self.literals.difference(&mss).copied().collect();
        Ok((mss, mcs))
    }

    /// Find a MUS by deleting literals from `unsat_subset`, until deleting any more leads to SAT.
    ///
    /// Optional optimization may allow skipping satisfiability checks for some deletions.
    fn shrink(&mut self, unsat_subset: &BTreeSet<Lit>, opti_mode: SubsetSolverOptiMode) -> Result<Mus, Exit> {
        let mut current = unsat_subset.clone();

        let mut skip = BTreeSet::<Lit>::new();
        self.shrink_optional_optimisation_lits_to_skip(opti_mode, &current, &mut skip);

        for &lit in unsat_subset {
            if !current.contains(&lit) || skip.contains(&lit) {
                continue;
            }
            current.remove(&lit);

            if let Err(unsat_core) = self.check_subset(&current)? {
                current = unsat_core;
                self.shrink_optional_optimisation_lits_to_skip(opti_mode, &current, &mut skip);
            } else {
                current.insert(lit);
            }
        }
        let mus = current;
        Ok(mus)
    }

    fn grow_optional_optimisation_lits_to_skip(
        &mut self,
        opti_mode: SubsetSolverOptiMode,
        current: &BTreeSet<Lit>,
        skip: &mut BTreeSet<Lit>,
    ) {
        match opti_mode {
            SubsetSolverOptiMode::None => (),
            SubsetSolverOptiMode::KnownSingletonMCSes => (),
            SubsetSolverOptiMode::KnownImplications => {
                // If some soft literals are found to be implied false by `current`,
                // then we know in advance that they can't possibly be in a MSS that includes `current`.
                // As such, we can skip inserting them in `current`, then calling `check_subset`,
                // and then removing them back from `current`.
                let implications = self.map_solver.known_implications(current);
                skip.clear();
                skip.extend(
                    implications
                        .iter()
                        .filter(|&&l| crate::core::Relation::Leq == l.relation()),
                );
            }
        }
    }

    fn shrink_optional_optimisation_lits_to_skip(
        &mut self,
        opti_mode: SubsetSolverOptiMode,
        current: &BTreeSet<Lit>,
        skip: &mut BTreeSet<Lit>,
    ) {
        match opti_mode {
            SubsetSolverOptiMode::None => (),
            SubsetSolverOptiMode::KnownSingletonMCSes => skip.extend(self.map_solver.known_singleton_mcses()),
            SubsetSolverOptiMode::KnownImplications => {
                // No literal from the complement of `current` can be in a MUS included in the unsat core `current`.
                // So if some literals are found to be implied true by
                // the whole complement of `current` being false,
                // then we know in advance that they are necessarily included in all unsat subsets of `current`,
                // i.e. in all MUSes included in `current`.
                // As such, we can skip removing them from `current`, then calling `check_subset`,
                // and then inserting them back into `current`.
                let implications = self
                    .map_solver
                    .known_implications(&self.literals.difference(current).map(|&l| !l).collect());
                skip.clear();
                skip.extend(
                    implications
                        .iter()
                        .filter(|&&l| crate::core::Relation::Gt == l.relation()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeSet;

    use itertools::Itertools;

    use crate::model::lang::expr::{geq, lt};
    use crate::solver::musmcs::marco::mapsolver::MapSolverMode;
    type Lbl = &'static str;

    type Model = crate::model::Model<Lbl>;
    type Solver = crate::solver::Solver<Lbl>;
    type Marco<'a> = crate::solver::musmcs::marco::Marco<'a, Lbl>;

    #[test]
    fn test_simple_marco_simple() {
        let mut solver = Solver::new(Model::new());
        let x0 = solver.model.new_ivar(0, 10, "x0");
        let x1 = solver.model.new_ivar(0, 10, "x1");
        let x2 = solver.model.new_ivar(0, 10, "x2");
        let x3 = solver.model.new_ivar(0, 10, "x3");

        let soft_constraints = vec![lt(x0, x1), lt(x1, x2), lt(x2, x0), lt(x0, x2), lt(x3, 5), geq(x3, 5)];
        let soft_constraints_reiflits = soft_constraints.iter().map(|sc| solver.half_reify(*sc)).collect_vec();

        let mut marco = Marco::with(
            soft_constraints_reiflits.iter().copied(),
            &mut solver,
            MapSolverMode::HighPreferredValues,
        );
        let res = marco.run(None, None);

        let res_muses = res.muses.into_iter().collect::<BTreeSet<_>>();
        let res_mcses = res.mcses.into_iter().collect::<BTreeSet<_>>();

        let expected_muses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[0],
                soft_constraints_reiflits[1],
                soft_constraints_reiflits[2],
            ]),
            BTreeSet::from_iter(vec![soft_constraints_reiflits[2], soft_constraints_reiflits[3]]),
            BTreeSet::from_iter(vec![soft_constraints_reiflits[4], soft_constraints_reiflits[5]]),
        ]);
        let expected_mcses = BTreeSet::from_iter(vec![
            BTreeSet::from_iter(vec![soft_constraints_reiflits[2], soft_constraints_reiflits[5]]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[0],
                soft_constraints_reiflits[3],
                soft_constraints_reiflits[5],
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[1],
                soft_constraints_reiflits[3],
                soft_constraints_reiflits[5],
            ]),
            BTreeSet::from_iter(vec![soft_constraints_reiflits[2], soft_constraints_reiflits[4]]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[0],
                soft_constraints_reiflits[3],
                soft_constraints_reiflits[4],
            ]),
            BTreeSet::from_iter(vec![
                soft_constraints_reiflits[1],
                soft_constraints_reiflits[3],
                soft_constraints_reiflits[4],
            ]),
        ]);

        assert_eq!(res_muses, expected_muses);
        assert_eq!(res_mcses, expected_mcses);
    }
}
