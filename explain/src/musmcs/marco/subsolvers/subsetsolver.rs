use std::collections::BTreeSet;
use std::sync::Arc;

use aries::core::Lit;
use aries::model::extensions::SavedAssignment;
use aries::model::{Label, Model};
use aries::reif::Reifiable;
use aries::solver::{Exit, UnsatCore};
use itertools::Itertools;

use crate::musmcs::marco::subsolvers::MapSolver;
use crate::musmcs::{Mcs, Mus};

/// A trait that allows defining the exact procedure
/// for solving / extracting unsat cores
/// to use by the subset solver in the MARCO algorithm.
pub trait SubsetSolverImpl<Lbl: Label> {
    fn get_model(&mut self) -> &mut Model<Lbl>;
    fn check_subset(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<Arc<SavedAssignment>, UnsatCore>, Exit>;
}

/// In theory, `KnownImplications` should be strictly better than `KnownSingletonMCSes`,
/// but the additional work needed to find these implications (involves propagations back and forth) could certainly be not worth it.
#[derive(Copy, Clone, Default)]
pub enum SubsetSolverOptiMode {
    None,
    #[default]
    KnownSingletonMCSes,
    KnownImplications,
}

pub(crate) struct SubsetSolver<Lbl: Label> {
    reiflits: BTreeSet<Lit>,
    solver_impl: Box<dyn SubsetSolverImpl<Lbl>>,
}

impl<Lbl: Label> SubsetSolver<Lbl> {
    pub fn new(reiflits: impl IntoIterator<Item = Lit>, mut solver_impl: Box<dyn SubsetSolverImpl<Lbl>>) -> Self {
        let reiflits = reiflits.into_iter().collect::<BTreeSet<Lit>>();
        assert!(
            reiflits
                .iter()
                .all(|&l| solver_impl.get_model().check_reified_any(l).is_some())
        );

        Self { reiflits, solver_impl }
    }

    pub fn get_expr_reif<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Option<Lit> {
        self.solver_impl.get_model().check_reified_any(expr)
    }

    pub fn get_soft_constraints_reif_lits(&self) -> &BTreeSet<Lit> {
        &self.reiflits
    }

    /// Checks whether the given subset of soft constraints (via their reification literals) is satisfiable.
    /// - If SAT: returns *all* soft constraint reification literals entailed in the found assignment (so a superset of `subset`).
    /// - If UNSAT: returns an unsat core of `subset`.
    pub fn check_subset(&mut self, subset: &BTreeSet<Lit>) -> Result<Result<BTreeSet<Lit>, BTreeSet<Lit>>, Exit> {
        // TODO Should return an annotation about the unsat core. To be customizable in the subset_solver_impl
        let res = match self.solver_impl.check_subset(subset)? {
            Ok(assignment) => Ok(self
                .reiflits
                .iter()
                .filter(|&&l| assignment.entails(l))
                .copied()
                .collect()),
            Err(unsat_core) => Err(unsat_core.literals().iter().copied().collect()),
        };
        Ok(res)
    }

    /// Find a MSS by adding soft constraints (reification literals) to `sat_subset`,
    /// until no more can be added without leading to UNSAT.
    ///
    /// Optional optimization may allow skipping satisfiability checks for some additions.
    pub fn grow(
        &mut self,
        sat_subset: &BTreeSet<Lit>,
        optimisation: (SubsetSolverOptiMode, &mut MapSolver),
    ) -> Result<(BTreeSet<Lit>, Mcs), Exit> {
        let sat_subset_complement = self.reiflits.difference(sat_subset).copied().collect_vec();
        let mut current = sat_subset.clone();

        // >>>>>>>> Optional Optimisation >>>>>>>> //
        let mut skip = BTreeSet::<Lit>::new();
        let (mode, msolver) = optimisation;
        match mode {
            SubsetSolverOptiMode::None => (),
            SubsetSolverOptiMode::KnownSingletonMCSes => (),
            SubsetSolverOptiMode::KnownImplications => {
                // If some soft constraint reification literals are found to be implied false by `current`,
                // then we know in advance that they can't possibly be in a MSS that includes `current`.
                // As such, we can skip inserting them in `current`, then calling `check_subset`,
                // and then removing them back from `current`.
                let implications = msolver.known_implications(&current);
                skip.clear();
                skip.extend(
                    implications
                        .iter()
                        .filter(|&&l| l.relation() == aries::core::Relation::Leq),
                );
            }
        }
        // <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<< //

        for lit in sat_subset_complement {
            if current.contains(&lit) || skip.contains(&lit) {
                continue;
            }
            current.insert(lit);

            if let Ok(superset) = self.check_subset(&current)? {
                current = superset;

                // >>>>>>>> Optional Optimisation >>>>>>>> //
                match mode {
                    SubsetSolverOptiMode::None => (),
                    SubsetSolverOptiMode::KnownSingletonMCSes => (),
                    SubsetSolverOptiMode::KnownImplications => {
                        // If some soft constraint reification literals are found to be implied false by `current`,
                        // then we know in advance that they can't possibly be in a MSS that includes `current`.
                        // As such, we can skip inserting them in `current`, then calling `check_subset`,
                        // and then removing them back from `current`.
                        let implications = msolver.known_implications(&current);
                        skip.clear();
                        skip.extend(
                            implications
                                .iter()
                                .filter(|&&l| l.relation() == aries::core::Relation::Leq),
                        );
                    }
                }
                // <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<< //
            } else {
                current.remove(&lit);
            }
        }
        let mss = current;
        let mcs = self.reiflits.difference(&mss).copied().collect();
        Ok((mss, mcs))
    }

    /// Find a MUS by deleting soft constraints (reification literals) from `unsat_subset`,
    /// until deleting any more leads to SAT.
    ///
    /// Optional optimization may allow skipping satisfiability checks for some deletions.
    pub fn shrink(
        &mut self,
        unsat_subset: &BTreeSet<Lit>,
        optimisation: (SubsetSolverOptiMode, &mut MapSolver),
    ) -> Result<Mus, Exit> {
        let mut current = unsat_subset.clone();

        // >>>>>>>> Optional Optimisation >>>>>>>> //
        let mut skip = BTreeSet::<Lit>::new();
        let (mode, msolver) = optimisation;
        match mode {
            SubsetSolverOptiMode::None => (),
            SubsetSolverOptiMode::KnownSingletonMCSes => skip.extend(msolver.known_singleton_mcses()),
            SubsetSolverOptiMode::KnownImplications => {
                // No literal from the complement of `current` can be in a MUS included in the unsat core `current`.
                // So if some soft constraint reification literals are found to be implied true by
                // the whole complement of `current` being false,
                // then we know in advance that they are necessarily included in all unsat subsets of `current`,
                // i.e. in all MUSes included in `current`.
                // As such, we can skip removing them from `current`, then calling `check_subset`,
                // and then inserting them back into `current`.
                let implications =
                    msolver.known_implications(&self.reiflits.difference(&current).map(|&l| !l).collect());
                skip.clear();
                skip.extend(
                    implications
                        .iter()
                        .filter(|&&l| l.relation() == aries::core::Relation::Gt),
                );
            }
        }
        // <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<< //

        for &lit in unsat_subset {
            if !current.contains(&lit) || skip.contains(&lit) {
                continue;
            }
            current.remove(&lit);

            if let Err(unsat_core) = self.check_subset(&current)? {
                current = unsat_core;

                // >>>>>>>> Optional Optimisation >>>>>>>> //
                match mode {
                    SubsetSolverOptiMode::None => (),
                    SubsetSolverOptiMode::KnownSingletonMCSes => skip.extend(msolver.known_singleton_mcses()),
                    SubsetSolverOptiMode::KnownImplications => {
                        // No literal from the complement of `current` can be in a MUS included in the unsat core `current`.
                        // So if some soft constraint reification literals are found to be implied true by
                        // the whole complement of `current` being false,
                        // then we know in advance that they are necessarily included in all unsat subsets of `current`,
                        // i.e. in all MUSes included in `current`.
                        // As such, we can skip removing them from `current`, then calling `check_subset`,
                        // and then inserting them back into `current`.
                        let implications =
                            msolver.known_implications(&self.reiflits.difference(&current).map(|&l| !l).collect());
                        skip.clear();
                        skip.extend(
                            implications
                                .iter()
                                .filter(|&&l| l.relation() == aries::core::Relation::Gt),
                        );
                    }
                }
                // <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<< ///
            } else {
                current.insert(lit);
            }
        }
        let mus = current;
        Ok(mus)
    }
}
