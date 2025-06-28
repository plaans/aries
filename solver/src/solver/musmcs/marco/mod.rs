mod mapsolver;

use crate::backtrack::{Backtrack, DecLvl};
use crate::core::Lit;
use crate::model::Label;
use crate::solver::{Exit, Solver};

use std::collections::BTreeSet;

use itertools::Itertools;

use crate::solver::musmcs::*;
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
    grow_shrink_optional_optimisation: SubsetSolverOptiMode,

    #[cfg(debug_assertions)]
    debug_found_muses: BTreeSet<BTreeSet<Lit>>,
    #[cfg(debug_assertions)]
    debug_found_mcses: BTreeSet<BTreeSet<Lit>>,
}

impl<'a, Lbl: Label> Iterator for Marco<'a, Lbl> {
    type Item = MusMcs;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO: return (non-minimal) Us/Cs-es when, for example, a timeout is reached or an Exit signal is received.
        self._next().map_or(None, |musmcs| musmcs)
    }
}

impl<'a, Lbl: Label> Marco<'a, Lbl> {
    #[cfg(not(debug_assertions))]
    pub fn with(
        literals: impl Iterator<Item = Lit> + Clone,
        main_solver: &'a mut Solver<Lbl>,
        map_solver_mode: MapSolverMode,
        main_solver_opti_mode: SubsetSolverOptiMode,
    ) -> Self {
        assert_eq!(main_solver.current_decision_level(), DecLvl::ROOT);
        let map_solver = MapSolver::new(literals.clone(), map_solver_mode);

        Self {
            literals: literals.into_iter().collect(),
            main_solver,
            map_solver,
            grow_shrink_optional_optimisation: main_solver_opti_mode,
        }
    }

    #[cfg(debug_assertions)]
    pub fn with(
        literals: impl Iterator<Item = Lit> + Clone,
        main_solver: &'a mut Solver<Lbl>,
        map_solver_mode: MapSolverMode,
        main_solver_opti_mode: SubsetSolverOptiMode,
    ) -> Self {
        assert_eq!(main_solver.current_decision_level(), DecLvl::ROOT);
        let map_solver = MapSolver::new(literals.clone(), map_solver_mode);

        Self {
            literals: literals.into_iter().collect(),
            main_solver,
            map_solver,
            grow_shrink_optional_optimisation: main_solver_opti_mode,
            debug_found_muses: BTreeSet::<_>::new(),
            debug_found_mcses: BTreeSet::<_>::new(),
        }
    }

    /// Provides an iterator over all MUS of the problem.
    pub fn mus_only(self) -> impl Iterator<Item = Mus> + use<'a, Lbl> {
        self.into_iter().filter_map(|mus_mcs| match mus_mcs {
            MusMcs::Mus(mus) => Some(mus),
            _ => None,
        })
    }

    /// Returns an arbitrary MUS (the first one encountered by the algorithm).
    pub fn first_mus(self) -> Option<Mus> {
        self.mus_only().next()
    }

    pub fn run(
        &mut self,
        on_mus_found: Option<fn(&BTreeSet<Lit>)>,
        on_mcs_found: Option<fn(&BTreeSet<Lit>)>,
    ) -> Vec<MusMcs> {
        let mut res = Vec::<MusMcs>::new();

        loop {
            // TODO: return (non-minimal) Us/Cs-es when, for example, a timeout is reached or an Exit signal is received.
            if let Ok(Some(musmcs)) = self._next() {
                match musmcs {
                    MusMcs::Mus(set) => {
                        on_mus_found.unwrap_or(|_| ())(&set);
                        res.push(MusMcs::Mus(set));
                    }
                    MusMcs::Mcs(set) => {
                        on_mcs_found.unwrap_or(|_| ())(&set);
                        res.push(MusMcs::Mcs(set));
                    }
                    _ => todo!(),
                }
            } else {
                return res;
            }
        }
    }

    fn _next(&mut self) -> Result<Option<MusMcs>, Exit> {
        if let Some(seed) = self.map_solver.find_unexplored_seed()? {
            if self.check_subset(&seed)?.is_ok() {
                let (_, mcs) = self.grow(&seed)?;
                self.map_solver.block_down(&mcs);

                if !mcs.is_empty() {
                    self.debug_check_mcs_is_new_and_correct(&mcs);
                    return Ok(Some(MusMcs::Mcs(mcs)));
                }
            } else {
                let mus = self.shrink(&seed)?;
                self.map_solver.block_up(&mus);

                if !mus.is_empty() {
                    self.debug_check_mus_is_new_and_correct(&mus);
                    return Ok(Some(MusMcs::Mus(mus)));
                }
            }
        }
        Ok(None)
    }
    #[cfg(not(debug_assertions))]
    #[expect(unreachable_code)]
    fn debug_check_mcs_is_new_and_correct(&mut self, _mcs: &BTreeSet<Lit>) {
        debug_assert!(unreachable!())
    }
    #[cfg(not(debug_assertions))]
    #[expect(unreachable_code)]
    fn debug_check_mus_is_new_and_correct(&mut self, _mus: &BTreeSet<Lit>) {
        debug_assert!(unreachable!())
    }
    #[cfg(debug_assertions)]
    fn debug_check_mcs_is_new_and_correct(&mut self, mcs: &BTreeSet<Lit>) {
        debug_assert!(self.debug_found_mcses.insert(mcs.clone()));

        assert_eq!(self.main_solver.current_decision_level(), DecLvl::ROOT);

        let mss = self.literals.difference(mcs).copied().collect_vec();
        let mcs = mcs.iter().copied().collect_vec();

        // Test mss being feasible
        debug_assert!(self.main_solver.solve_with_assumptions(&mss).unwrap().is_ok());
        // Test mcs being minimal (i.e. mss + any element of mcs being infeasible)
        for &lit in &mcs {
            self.main_solver.reset();
            debug_assert!(self
                .main_solver
                .solve_with_assumptions(&mss.iter().chain([&lit]).copied().collect_vec())
                .unwrap()
                .is_err())
        }
        self.main_solver.reset();
    }
    #[cfg(debug_assertions)]
    fn debug_check_mus_is_new_and_correct(&mut self, mus: &BTreeSet<Lit>) {
        debug_assert!(self.debug_found_muses.insert(mus.clone()));

        assert_eq!(self.main_solver.current_decision_level(), DecLvl::ROOT);

        let mus = mus.iter().copied().collect_vec();

        // Test mus being infeasible
        debug_assert!(self.main_solver.solve_with_assumptions(&mus).unwrap().is_err());

        // Test mus being minimal
        for &lit in &mus {
            self.main_solver.reset();
            debug_assert!(self
                .main_solver
                .solve_with_assumptions(&mus.iter().filter_map(|&l| (l != lit).then_some(l)).collect_vec())
                .unwrap()
                .is_ok());
        }
        self.main_solver.reset();
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
    fn grow(&mut self, sat_subset: &BTreeSet<Lit>) -> Result<(BTreeSet<Lit>, BTreeSet<Lit>), Exit> {
        let sat_subset_complement = self.literals.difference(sat_subset).copied().collect_vec();
        let mut current = sat_subset.clone();

        let mut skip = BTreeSet::<Lit>::new();
        self.grow_optional_optimisation_lits_to_skip(&current, &mut skip);

        for lit in sat_subset_complement {
            if current.contains(&lit) || skip.contains(&lit) {
                continue;
            }
            current.insert(lit);

            if let Ok(superset) = self.check_subset(&current)? {
                current = superset;
                self.grow_optional_optimisation_lits_to_skip(&current, &mut skip);
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
    fn shrink(&mut self, unsat_subset: &BTreeSet<Lit>) -> Result<BTreeSet<Lit>, Exit> {
        let mut current = unsat_subset.clone();

        let mut skip = BTreeSet::<Lit>::new();
        self.shrink_optional_optimisation_lits_to_skip(&current, &mut skip);

        for &lit in unsat_subset {
            if !current.contains(&lit) || skip.contains(&lit) {
                continue;
            }
            current.remove(&lit);

            if let Err(unsat_core) = self.check_subset(&current)? {
                current = unsat_core;
                self.shrink_optional_optimisation_lits_to_skip(&current, &mut skip);
            } else {
                current.insert(lit);
            }
        }
        let mus = current;
        Ok(mus)
    }

    fn grow_optional_optimisation_lits_to_skip(&mut self, current: &BTreeSet<Lit>, skip: &mut BTreeSet<Lit>) {
        match self.grow_shrink_optional_optimisation {
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

    fn shrink_optional_optimisation_lits_to_skip(&mut self, current: &BTreeSet<Lit>, skip: &mut BTreeSet<Lit>) {
        match self.grow_shrink_optional_optimisation {
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

    use crate::core::Lit;
    use crate::model::lang::expr::{geq, lt};
    use crate::solver::musmcs::marco::mapsolver::MapSolverMode;
    use crate::solver::musmcs::MusMcs;
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
            crate::solver::musmcs::marco::SubsetSolverOptiMode::None,
        );
        let res = marco.run(None, None);

        let mut res_muses = BTreeSet::<BTreeSet<Lit>>::new();
        let mut res_mcses = BTreeSet::<BTreeSet<Lit>>::new();

        for musmcs in res {
            match musmcs {
                MusMcs::Mus(set) => {
                    res_muses.insert(set);
                }
                MusMcs::Mcs(set) => {
                    res_mcses.insert(set);
                }
                _ => panic!(),
            }
        }

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
